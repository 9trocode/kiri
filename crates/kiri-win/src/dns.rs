//! `<name>.kiri.local` on Windows, as /etc/resolver/kiri.local is on the
//! Mac: a name-resolution policy (NRPT) rule sends `.kiri.local` lookups to
//! this small DNS server on 127.0.0.1:53, which answers 127.0.0.1, where
//! the daemon's router sorts requests out by name. Adding or removing the
//! rule is the only step that needs an administrator; Windows asks once.

use std::io;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};

const SUFFIX: &str = "kiri.local";
/// Where Windows keeps local NRPT rules.
const RULES: &str = r"HKLM\SYSTEM\CurrentControlSet\Services\Dnscache\Parameters\DnsPolicyConfig";

/// Whether the answerer holds 127.0.0.1:53. Without it the NRPT rule sends
/// `.kiri.local` lookups to nobody, so Domains reads as off.
static SERVING: AtomicBool = AtomicBool::new(false);

pub fn serving() -> bool {
    SERVING.load(Ordering::Relaxed)
}

/// Answers on 127.0.0.1:53 on a thread of its own. Windows' Internet
/// Connection Sharing holds 0.0.0.0:53, which the loopback address still
/// wins over.
pub fn start() -> io::Result<()> {
    let socket = bind_exclusive()?;
    SERVING.store(true, Ordering::Relaxed);
    std::thread::Builder::new().name("kiri-dns".into()).spawn(move || {
        let mut buf = [0u8; 512];
        loop {
            let Ok((n, from)) = socket.recv_from(&mut buf) else { continue };
            if let Some(reply) = answer(&buf[..n]) {
                let _ = socket.send_to(&reply, from);
            }
        }
    })?;
    Ok(())
}

/// 127.0.0.1:53 with SO_EXCLUSIVEADDRUSE: Windows otherwise lets another
/// program bind the same address later and take the queries.
fn bind_exclusive() -> io::Result<UdpSocket> {
    use std::os::windows::io::FromRawSocket;
    use windows_sys::Win32::Networking::WinSock::{
        bind, closesocket, setsockopt, socket, WSAStartup, AF_INET, INVALID_SOCKET, IN_ADDR, IN_ADDR_0, IPPROTO_UDP, SOCKADDR, SOCKADDR_IN, SOCK_DGRAM,
        SOL_SOCKET, SO_REUSEADDR, WSADATA,
    };
    // A macro in winsock2.h, so not among the crate's constants.
    const SO_EXCLUSIVEADDRUSE: i32 = !SO_REUSEADDR;
    // SAFETY: plain Winsock calls; the socket is closed on every failure and
    // owned by the UdpSocket on success.
    unsafe {
        let mut data: WSADATA = std::mem::zeroed();
        if WSAStartup(0x0202, &mut data) != 0 {
            return Err(io::Error::other("Winsock did not start"));
        }
        let s = socket(i32::from(AF_INET), SOCK_DGRAM, IPPROTO_UDP);
        if s == INVALID_SOCKET {
            return Err(io::Error::last_os_error());
        }
        let one: i32 = 1;
        let addr = SOCKADDR_IN {
            sin_family: AF_INET,
            sin_port: 53u16.to_be(),
            sin_addr: IN_ADDR { S_un: IN_ADDR_0 { S_addr: u32::from_ne_bytes([127, 0, 0, 1]) } },
            sin_zero: [0; 8],
        };
        if setsockopt(s, SOL_SOCKET, SO_EXCLUSIVEADDRUSE, (&one as *const i32).cast(), 4) != 0
            || bind(s, (&addr as *const SOCKADDR_IN).cast::<SOCKADDR>(), std::mem::size_of::<SOCKADDR_IN>() as i32) != 0
        {
            let e = io::Error::last_os_error();
            closesocket(s);
            return Err(e);
        }
        Ok(UdpSocket::from_raw_socket(s as std::os::windows::io::RawSocket))
    }
}

/// The reply to one query: 127.0.0.1 for an A record under kiri.local, no
/// records for its other types, and REFUSED for any other name.
fn answer(q: &[u8]) -> Option<Vec<u8>> {
    if q.len() < 12 || q[2] & 0x80 != 0 || u16::from_be_bytes([q[4], q[5]]) != 1 {
        return None;
    }
    // The question: labels, then type and class.
    let mut i = 12;
    let mut labels = Vec::new();
    loop {
        let len = *q.get(i)? as usize;
        i += 1;
        if len == 0 {
            break;
        }
        if len > 63 {
            return None;
        }
        labels.push(String::from_utf8_lossy(q.get(i..i + len)?).to_ascii_lowercase());
        i += len;
    }
    let qtype = u16::from_be_bytes([*q.get(i)?, *q.get(i + 1)?]);
    let question = q.get(12..i + 4)?;
    let name = labels.join(".");
    let ours = name == SUFFIX || name.ends_with(&format!(".{SUFFIX}"));
    let a = ours && qtype == 1;
    let mut r = Vec::with_capacity(question.len() + 28);
    r.extend_from_slice(&q[0..2]);
    // Response, the query's opcode and RD, authoritative; REFUSED if not ours.
    r.push(0x80 | (q[2] & 0x79) | 0x04);
    r.push(if ours { 0 } else { 5 });
    r.extend_from_slice(&[0, 1, 0, u8::from(a), 0, 0, 0, 0]);
    r.extend_from_slice(question);
    if a {
        // The name (a pointer to the question's), A, IN, TTL 5 s, 127.0.0.1.
        r.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1, 0, 0, 0, 5, 0, 4, 127, 0, 0, 1]);
    }
    Some(r)
}

/// Whether Windows sends `.kiri.local` lookups here.
pub fn installed() -> bool {
    let mut cmd = crate::win::tool("reg");
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
    cmd.args(["query", RULES, "/s", "/f", &format!(".{SUFFIX}"), "/d"]).output().is_ok_and(|o| {
        let out = String::from_utf8_lossy(&o.stdout).to_ascii_lowercase();
        out.lines().any(|l| l.contains("name") && l.contains(&format!(".{SUFFIX}")))
    })
}

/// Adds (or removes) the rule, through an elevated PowerShell: Windows asks
/// for an administrator's approval. A refusal is an error whose message
/// says "canceled". The PowerShell runs kiri-admin.ps1 -Dns (contrib/windows,
/// installed beside this exe and checked against this build's SHA-256), with
/// the suffix and -Off as arguments. A script file, not one built here and
/// run with -EncodedCommand: an elevated, hidden PowerShell fed an encoded
/// script is what malware droppers do, and Defender quarantined a
/// kiri-win.exe that carried it (Trojan:Win32/Bearfoos.A!ml).
pub fn install(on: bool) -> io::Result<()> {
    let script = crate::setup::verified("kiri-admin.ps1")?;
    let mut args = vec!["-Dns", "-Suffix", SUFFIX];
    if !on {
        args.push("-Off");
    }
    match crate::win::elevated_script(&script, &args) {
        Ok(0) => Ok(()),
        // As before: a declined approval, or anything else that kept it from starting.
        Ok(2) | Err(_) => Err(io::Error::other("canceled")),
        Ok(_) => Err(io::Error::other(format!("the DNS rule could not be {}", if on { "added" } else { "removed" }))),
    }
}

#[cfg(test)]
mod tests {
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
        let r = super::answer(&query("Web.Kiri.Local", 1)).unwrap();
        assert_eq!(&r[0..2], &[0x12, 0x34]);
        assert_eq!(r[3] & 0x0f, 0);
        assert_eq!(&r[r.len() - 4..], &[127, 0, 0, 1]);
        let aaaa = super::answer(&query("web.kiri.local", 28)).unwrap();
        assert_eq!(u16::from_be_bytes([aaaa[6], aaaa[7]]), 0);
        let other = super::answer(&query("example.com", 1)).unwrap();
        assert_eq!(other[3] & 0x0f, 5);
        assert!(super::answer(&query("evil.kiri.local.example.com", 1)).unwrap()[3] & 0x0f == 5);
    }
}
