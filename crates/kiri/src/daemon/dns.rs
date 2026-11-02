//! `*.kiri.local`: a tiny DNS server on loopback that answers every name
//! under `kiri.local` with 127.0.0.1, where the domain router (domains.rs)
//! picks the container from the Host header. macOS sends these lookups here
//! because of `/etc/resolver/kiri.local`, which `kiri setup` installs once
//! (it needs an administrator). Anything else is refused.

use std::io;
use std::net::UdpSocket;

/// The resolver file's port; unprivileged, so the daemon needs no root.
pub const PORT: u16 = 53535;
pub const RESOLVER: &str = "/etc/resolver/kiri.local";
pub const SUFFIX: &str = "kiri.local";

pub fn start() -> io::Result<()> {
    let sock = UdpSocket::bind(("127.0.0.1", PORT))?;
    std::thread::Builder::new().name("kiri-dns".into()).spawn(move || {
        let mut buf = [0u8; 512];
        loop {
            let Ok((n, from)) = sock.recv_from(&mut buf) else { continue };
            if let Some(reply) = answer(&buf[..n]) {
                let _ = sock.send_to(&reply, from);
            }
        }
    })?;
    Ok(())
}

/// True when macOS will send `*.kiri.local` lookups to Kiri.
pub fn installed() -> bool {
    std::fs::read_to_string(RESOLVER).is_ok_and(|s| s.contains(&format!("port {PORT}")))
}

/// One reply for one query: A → 127.0.0.1 under kiri.local, an empty answer
/// for other types there (so AAAA falls back to A), REFUSED elsewhere.
fn answer(q: &[u8]) -> Option<Vec<u8>> {
    if q.len() < 12 || q[2] & 0x80 != 0 || u16::from_be_bytes([q[4], q[5]]) != 1 {
        return None;
    }
    // The question name, as labels.
    let mut i = 12;
    let mut name = String::new();
    loop {
        let len = *q.get(i)? as usize;
        i += 1;
        if len == 0 {
            break;
        }
        if len > 63 {
            return None;
        }
        let label = q.get(i..i + len)?;
        if !name.is_empty() {
            name.push('.');
        }
        name.push_str(&String::from_utf8_lossy(label).to_ascii_lowercase());
        i += len;
    }
    let qtype = u16::from_be_bytes([*q.get(i)?, *q.get(i + 1)?]);
    let question = q.get(12..i + 4)?;
    let ours = name == SUFFIX || name.ends_with(&format!(".{SUFFIX}"));
    let a = ours && qtype == 1;
    let mut r = Vec::with_capacity(question.len() + 28);
    r.extend_from_slice(&q[..2]);
    // QR, AA, RD copied; RCODE 5 (REFUSED) when the name is not ours.
    r.extend_from_slice(&[0x84 | (q[2] & 0x01), if ours { 0 } else { 5 }]);
    r.extend_from_slice(&[0, 1, 0, a as u8, 0, 0, 0, 0]);
    r.extend_from_slice(question);
    if a {
        // A pointer to the question name, class IN, TTL 60, 127.0.0.1.
        r.extend_from_slice(&[0xc0, 12, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4, 127, 0, 0, 1]);
    }
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::answer;

    fn query(name: &str, qtype: u16) -> Vec<u8> {
        let mut q = vec![0x12, 0x34, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
        for l in name.split('.') {
            q.push(l.len() as u8);
            q.extend_from_slice(l.as_bytes());
        }
        q.push(0);
        q.extend_from_slice(&qtype.to_be_bytes());
        q.extend_from_slice(&[0, 1]);
        q
    }

    #[test]
    fn answers_kiri_local_only() {
        let r = answer(&query("Web.kiri.local", 1)).unwrap();
        assert_eq!((&r[..2], r[3], r[7]), (&[0x12, 0x34][..], 0, 1));
        assert_eq!(&r[r.len() - 4..], &[127, 0, 0, 1]);
        let aaaa = answer(&query("web.kiri.local", 28)).unwrap();
        assert_eq!((aaaa[3], aaaa[7]), (0, 0));
        assert_eq!(answer(&query("example.com", 1)).unwrap()[3], 5);
        assert_eq!(answer(&query("notkiri.local", 1)).unwrap()[3], 5);
        assert!(answer(&[0; 5]).is_none());
    }
}
