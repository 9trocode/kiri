//! Wire protocol between `kiri share` and the Kiri Share edge.
//!
//! The client dials the edge over TLS on port 443 (ALPN [`ALPN`]) and keeps
//! that one connection open. Each visitor connection the edge accepts for the
//! tunnel becomes a numbered stream inside it, multiplexed with [`Msg::Data`]
//! frames and flow-controlled with [`Msg::Window`] credit, so one slow
//! visitor can never hold up another.
//!
//! Every frame is `len: u32 LE | kind: u16 LE | payload`, where `len` counts
//! `kind` plus `payload`, as in `kiri-proto`. Strings are `u32 len | bytes`.
//! Encoding appends to a caller-owned buffer and decoding borrows from one.
//!
//! Session: client sends `Hello`; the edge answers `Welcome` or `Refused`.
//! A client that sets [`FEATURE_ATTEST`] in its `Hello` may first get a
//! `Challenge`, which it answers with `Attest` (see [`attest`]); clients that
//! don't set it are never challenged, so older clients work unchanged.
//! Then the edge sends `Open` per visitor connection; both sides exchange
//! `Data`, `Window`, `Close` (half-close) and `Reset` for that stream. Either
//! side may `Ping`. The edge ends a session with `Goodbye`.
//!
//! Lanes: a client that sets [`FEATURE_LANES`] may be told in `Welcome` it
//! can open `lanes` more connections. Each starts with `Join` (the
//! `lane_key` from `Welcome`) and gets `Joined`. Every stream still belongs
//! to the first connection, but the client sends a stream's bytes as
//! numbered `Chunk`s spread over all of them, so one lossy connection
//! doesn't cap the tunnel; the edge puts them back in order. A `Chunk` with
//! `fin` ends the stream's data, like `Close`. Streams then use `window`
//! from `Welcome` instead of [`INITIAL_WINDOW`] for what the client sends.
//! A client that sets [`FEATURE_WINDOW`] uses that window with or without
//! lanes; others only with lanes, so an edge must not send a window larger
//! than [`INITIAL_WINDOW`] to them on a session without lanes.

#![forbid(unsafe_code)]
#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::vec::Vec;

pub mod attest;

pub const PROTOCOL: u16 = 1;

/// `Hello::features` bit: this client can answer a `Challenge`, because it
/// is an official build with a build key compiled in.
pub const FEATURE_ATTEST: u32 = 1;

/// `Hello::features` bit: this client can open lanes (see the module docs).
pub const FEATURE_LANES: u32 = 2;

/// `Hello::features` bit: this client uses `Welcome::window` even when
/// `Welcome::lanes` is 0, so the edge may give a one-connection session the
/// account's window. Without it the edge should send 0 there.
pub const FEATURE_WINDOW: u32 = 4;

/// ALPN the client offers, so the edge can tell tunnel clients from visitors
/// on the same port.
pub const ALPN: &[u8] = b"kiri-share/1";

/// Bytes before the payload: length plus kind.
pub const HEADER: usize = 6;

/// Largest `Data` payload a sender puts in one frame.
pub const MAX_CHUNK: usize = 32 * 1024;

/// Largest frame either side accepts; anything bigger is a protocol error.
pub const MAX_FRAME: usize = 64 * 1024;

/// Bytes a sender may have in flight on a new stream before the receiver
/// grants more with `Window`. Bounds the memory one stream can pin.
pub const INITIAL_WINDOW: u32 = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg<'a> {
    /// Client → edge, first frame.
    Hello(Hello<'a>),
    /// Edge → client: the tunnel is live.
    Welcome(Welcome<'a>),
    /// Edge → client: no tunnel; the edge closes the connection after this.
    Refused(&'a str),
    /// Edge → client: a visitor connected; open the local service.
    Open { stream: u32, remote: &'a str },
    Data { stream: u32, data: &'a [u8] },
    /// The receiver consumed `credit` more bytes of this stream.
    Window { stream: u32, credit: u32 },
    /// The sender will send no more data on this stream.
    Close { stream: u32 },
    /// The stream is gone in both directions.
    Reset { stream: u32 },
    Ping(u64),
    Pong(u64),
    /// Edge → client: something the person running the tunnel should see.
    Notice(&'a str),
    /// Edge → client: the session is over. With `reconnect` the client should
    /// dial again (the edge is restarting); without it, it should stop.
    Goodbye { reconnect: bool, reason: &'a str },
    /// Edge → client, after a `Hello` with [`FEATURE_ATTEST`]: prove this is
    /// an official build by answering with `Attest`.
    Challenge { nonce: [u8; attest::NONCE_LEN] },
    /// Client → edge, the answer to `Challenge`: which build key (its id, as
    /// configured on the edge) and [`attest::proof`] made with it.
    Attest { key: &'a str, mac: [u8; attest::MAC_LEN] },
    /// Client → edge, the first frame on an extra connection: the
    /// `lane_key` from `Welcome`.
    Join { key: &'a str },
    /// Edge → client: the lane is open.
    Joined,
    /// Client → edge: stream bytes numbered from 0, on any lane. `fin`
    /// ends the stream's data after `data`.
    Chunk { stream: u32, seq: u32, fin: bool, data: &'a [u8] },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hello<'a> {
    pub protocol: u16,
    /// The account token from `kiri share login`.
    pub token: &'a str,
    /// Random id of this Kiri install, kept in `~/.kiri/share.json`.
    pub device: &'a str,
    /// Preferred first label of the hostname, such as a container name. The
    /// edge sanitises it and adds a random suffix.
    pub name: &'a str,
    /// What the tunnel points at, for the owner's dashboard and abuse review.
    pub target: &'a str,
    /// Client version, such as `kiri 0.1.0`.
    pub client: &'a str,
    /// `FEATURE_*` bits. Sent after the other fields and read as 0 when a
    /// client leaves it out, so older clients and edges interoperate.
    pub features: u32,
    /// Stable id of the computer, a hash of its hardware id: the same for
    /// the app and `kiri share` on one Mac, whatever their config dirs.
    /// After `features`, read as empty when left out, like `host`.
    pub machine: &'a str,
    /// The computer's name as people know it, such as `paul-mbp`.
    pub host: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Welcome<'a> {
    pub url: &'a str,
    /// Bytes per second each direction may use; 0 means no limit.
    pub rate: u32,
    /// Bytes left this month; `u64::MAX` means no cap.
    pub remaining: u64,
    /// Browsers see a warning page before the site.
    pub warning: bool,
    /// The tunnel's verified custom domains as URLs, comma-separated; empty
    /// for none. Sent after the other fields and read as empty when an edge
    /// leaves it out; older clients stop reading before it.
    pub domains: &'a str,
    /// Extra connections the client may open; 0 for none. After `domains`,
    /// read as 0 when left out, like `lane_key` and `window`.
    pub lanes: u32,
    /// What a lane sends in `Join`.
    pub lane_key: &'a str,
    /// Bytes the client may have in flight per stream; 0 means
    /// [`INITIAL_WINDOW`].
    pub window: u32,
}

impl Welcome<'_> {
    /// Bytes the client may have in flight on one stream.
    pub fn send_window(&self) -> u32 {
        if self.window == 0 { INITIAL_WINDOW } else { self.window }
    }
}

mod kind {
    pub const HELLO: u16 = 1;
    pub const WELCOME: u16 = 2;
    pub const REFUSED: u16 = 3;
    pub const OPEN: u16 = 4;
    pub const DATA: u16 = 5;
    pub const WINDOW: u16 = 6;
    pub const CLOSE: u16 = 7;
    pub const RESET: u16 = 8;
    pub const PING: u16 = 9;
    pub const PONG: u16 = 10;
    pub const NOTICE: u16 = 11;
    pub const GOODBYE: u16 = 12;
    pub const CHALLENGE: u16 = 13;
    pub const ATTEST: u16 = 14;
    pub const JOIN: u16 = 15;
    pub const JOINED: u16 = 16;
    pub const CHUNK: u16 = 17;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    TooLarge(usize),
    UnknownKind(u16),
    Truncated,
    BadUtf8,
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::TooLarge(n) => write!(f, "frame of {n} bytes is too large"),
            DecodeError::UnknownKind(k) => write!(f, "unknown frame kind {k}"),
            DecodeError::Truncated => f.write_str("truncated frame"),
            DecodeError::BadUtf8 => f.write_str("string is not UTF-8"),
        }
    }
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u32(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}

fn frame(out: &mut Vec<u8>, kind: u16, body: impl FnOnce(&mut Vec<u8>)) {
    let start = out.len();
    out.extend_from_slice(&[0; HEADER]);
    body(out);
    let len = (out.len() - start - 4) as u32;
    out[start..start + 4].copy_from_slice(&len.to_le_bytes());
    out[start + 4..start + HEADER].copy_from_slice(&kind.to_le_bytes());
}

impl Msg<'_> {
    /// Appends one encoded frame to `out`.
    pub fn encode(&self, out: &mut Vec<u8>) {
        match *self {
            Msg::Hello(h) => frame(out, kind::HELLO, |o| {
                o.extend_from_slice(&h.protocol.to_le_bytes());
                for s in [h.token, h.device, h.name, h.target, h.client] {
                    put_str(o, s);
                }
                put_u32(o, h.features);
                put_str(o, h.machine);
                put_str(o, h.host);
            }),
            Msg::Welcome(w) => frame(out, kind::WELCOME, |o| {
                put_str(o, w.url);
                put_u32(o, w.rate);
                o.extend_from_slice(&w.remaining.to_le_bytes());
                o.push(w.warning as u8);
                put_str(o, w.domains);
                put_u32(o, w.lanes);
                put_str(o, w.lane_key);
                put_u32(o, w.window);
            }),
            Msg::Refused(r) => frame(out, kind::REFUSED, |o| o.extend_from_slice(r.as_bytes())),
            Msg::Open { stream, remote } => frame(out, kind::OPEN, |o| {
                put_u32(o, stream);
                o.extend_from_slice(remote.as_bytes());
            }),
            Msg::Data { stream, data } => frame(out, kind::DATA, |o| {
                put_u32(o, stream);
                o.extend_from_slice(data);
            }),
            Msg::Window { stream, credit } => frame(out, kind::WINDOW, |o| {
                put_u32(o, stream);
                put_u32(o, credit);
            }),
            Msg::Close { stream } => frame(out, kind::CLOSE, |o| put_u32(o, stream)),
            Msg::Reset { stream } => frame(out, kind::RESET, |o| put_u32(o, stream)),
            Msg::Ping(v) => frame(out, kind::PING, |o| o.extend_from_slice(&v.to_le_bytes())),
            Msg::Pong(v) => frame(out, kind::PONG, |o| o.extend_from_slice(&v.to_le_bytes())),
            Msg::Notice(n) => frame(out, kind::NOTICE, |o| o.extend_from_slice(n.as_bytes())),
            Msg::Goodbye { reconnect, reason } => frame(out, kind::GOODBYE, |o| {
                o.push(reconnect as u8);
                o.extend_from_slice(reason.as_bytes());
            }),
            Msg::Challenge { nonce } => frame(out, kind::CHALLENGE, |o| o.extend_from_slice(&nonce)),
            Msg::Attest { key, mac } => frame(out, kind::ATTEST, |o| {
                put_str(o, key);
                o.extend_from_slice(&mac);
            }),
            Msg::Join { key } => frame(out, kind::JOIN, |o| o.extend_from_slice(key.as_bytes())),
            Msg::Joined => frame(out, kind::JOINED, |_| {}),
            Msg::Chunk { stream, seq, fin, data } => frame(out, kind::CHUNK, |o| {
                put_u32(o, stream);
                put_u32(o, seq);
                o.push(fin as u8);
                o.extend_from_slice(data);
            }),
        }
    }
}

/// Bytes before a `Chunk`'s data: header, stream, seq and fin.
pub const CHUNK_HEADER: usize = HEADER + 9;

/// Encodes a `Chunk` frame header for `len` data bytes, like [`data_header`].
pub fn chunk_header(stream: u32, seq: u32, fin: bool, len: usize) -> [u8; CHUNK_HEADER] {
    let mut h = [0; CHUNK_HEADER];
    h[..4].copy_from_slice(&((2 + 9 + len) as u32).to_le_bytes());
    h[4..6].copy_from_slice(&kind::CHUNK.to_le_bytes());
    h[6..10].copy_from_slice(&stream.to_le_bytes());
    h[10..14].copy_from_slice(&seq.to_le_bytes());
    h[14] = fin as u8;
    h
}

/// Encodes a `Data` frame header for `len` payload bytes, so a sender can
/// read straight into the buffer after it instead of copying.
pub fn data_header(stream: u32, len: usize) -> [u8; HEADER + 4] {
    let mut h = [0; HEADER + 4];
    h[..4].copy_from_slice(&((2 + 4 + len) as u32).to_le_bytes());
    h[4..6].copy_from_slice(&kind::DATA.to_le_bytes());
    h[6..].copy_from_slice(&stream.to_le_bytes());
    h
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.pos.checked_add(n).ok_or(DecodeError::Truncated)?;
        let s = self.buf.get(self.pos..end).ok_or(DecodeError::Truncated)?;
        self.pos = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().map_err(|_| DecodeError::Truncated)?))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| DecodeError::Truncated)?))
    }

    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(|_| DecodeError::Truncated)?))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        self.take(N)?.try_into().map_err(|_| DecodeError::Truncated)
    }

    /// A trailing string that older peers leave out: empty when absent.
    fn opt_str(&mut self) -> Result<&'a str, DecodeError> {
        if self.pos == self.buf.len() { Ok("") } else { self.str() }
    }

    /// A trailing `u32` that older peers leave out: 0 when absent.
    fn opt_u32(&mut self) -> Result<u32, DecodeError> {
        if self.pos == self.buf.len() { Ok(0) } else { self.u32() }
    }

    fn str(&mut self) -> Result<&'a str, DecodeError> {
        let n = self.u32()? as usize;
        core::str::from_utf8(self.take(n)?).map_err(|_| DecodeError::BadUtf8)
    }

    fn rest(&mut self) -> &'a [u8] {
        let s = &self.buf[self.pos..];
        self.pos = self.buf.len();
        s
    }

    fn rest_str(&mut self) -> Result<&'a str, DecodeError> {
        core::str::from_utf8(self.rest()).map_err(|_| DecodeError::BadUtf8)
    }
}

/// Returns the total size of the frame whose 4-byte length prefix is
/// `header`, so a reader can size exactly one read for the rest.
pub fn frame_len(header: &[u8; 4]) -> Result<usize, DecodeError> {
    let len = u32::from_le_bytes(*header) as usize;
    if len < 2 || len + 4 > MAX_FRAME {
        return Err(DecodeError::TooLarge(len));
    }
    Ok(len + 4)
}

/// Decodes one complete frame (header included).
pub fn decode(frame: &[u8]) -> Result<Msg<'_>, DecodeError> {
    if frame.len() < HEADER {
        return Err(DecodeError::Truncated);
    }
    let kind = u16::from_le_bytes([frame[4], frame[5]]);
    let mut r = Reader { buf: &frame[HEADER..], pos: 0 };
    Ok(match kind {
        kind::HELLO => Msg::Hello(Hello {
            protocol: r.u16()?,
            token: r.str()?,
            device: r.str()?,
            name: r.str()?,
            target: r.str()?,
            client: r.str()?,
            features: r.opt_u32()?,
            machine: r.opt_str()?,
            host: r.opt_str()?,
        }),
        kind::WELCOME => Msg::Welcome(Welcome {
            url: r.str()?,
            rate: r.u32()?,
            remaining: r.u64()?,
            warning: r.u8()? != 0,
            domains: r.opt_str()?,
            lanes: r.opt_u32()?,
            lane_key: r.opt_str()?,
            window: r.opt_u32()?,
        }),
        kind::REFUSED => Msg::Refused(r.rest_str()?),
        kind::OPEN => Msg::Open { stream: r.u32()?, remote: r.rest_str()? },
        kind::DATA => Msg::Data { stream: r.u32()?, data: r.rest() },
        kind::WINDOW => Msg::Window { stream: r.u32()?, credit: r.u32()? },
        kind::CLOSE => Msg::Close { stream: r.u32()? },
        kind::RESET => Msg::Reset { stream: r.u32()? },
        kind::PING => Msg::Ping(r.u64()?),
        kind::PONG => Msg::Pong(r.u64()?),
        kind::NOTICE => Msg::Notice(r.rest_str()?),
        kind::GOODBYE => Msg::Goodbye { reconnect: r.u8()? != 0, reason: r.rest_str()? },
        kind::CHALLENGE => Msg::Challenge { nonce: r.array()? },
        kind::ATTEST => Msg::Attest { key: r.str()?, mac: r.array()? },
        kind::JOIN => Msg::Join { key: r.rest_str()? },
        kind::JOINED => Msg::Joined,
        kind::CHUNK => Msg::Chunk { stream: r.u32()?, seq: r.u32()?, fin: r.u8()? != 0, data: r.rest() },
        k => return Err(DecodeError::UnknownKind(k)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(m: Msg<'_>) {
        let mut buf = Vec::new();
        m.encode(&mut buf);
        assert_eq!(frame_len(buf[..4].try_into().unwrap()).unwrap(), buf.len());
        assert_eq!(decode(&buf).unwrap(), m);
    }

    #[test]
    fn round_trip() {
        round(Msg::Hello(Hello { protocol: PROTOCOL, token: "tok", device: "dev", name: "web", target: "localhost:3000", client: "kiri 0.1.0", features: 0, machine: "", host: "" }));
        round(Msg::Hello(Hello { protocol: PROTOCOL, token: "tok", device: "dev", name: "web", target: "localhost:3000", client: "kiri 0.1.0", features: FEATURE_ATTEST, machine: "", host: "" }));
        round(Msg::Hello(Hello { protocol: PROTOCOL, token: "tok", device: "dev", name: "web", target: "localhost:3000", client: "Kiri Dev 0.9.4", features: FEATURE_ATTEST, machine: "3f9a0c41d2e87b65", host: "paul-mbp" }));
        round(Msg::Welcome(Welcome { url: "https://web-x7k2.runkiri.cloud", rate: 512 * 1024, remaining: u64::MAX, warning: true, domains: "", lanes: 0, lane_key: "", window: 0 }));
        round(Msg::Welcome(Welcome { url: "https://web.runkiri.cloud", rate: 0, remaining: u64::MAX, warning: false, domains: "https://dev.example.com,https://b.example.org", lanes: 0, lane_key: "", window: 0 }));
        round(Msg::Welcome(Welcome { url: "https://web.runkiri.cloud", rate: 0, remaining: u64::MAX, warning: false, domains: "", lanes: 3, lane_key: "k3y", window: 1 << 20 }));
        round(Msg::Join { key: "k3y" });
        round(Msg::Joined);
        round(Msg::Chunk { stream: 7, seq: 0, fin: false, data: b"hello" });
        round(Msg::Chunk { stream: 7, seq: 9, fin: true, data: b"" });
        round(Msg::Refused("banned"));
        round(Msg::Open { stream: 7, remote: "203.0.113.9:51234" });
        round(Msg::Data { stream: 7, data: b"GET / HTTP/1.1\r\n\r\n" });
        round(Msg::Data { stream: 7, data: b"" });
        round(Msg::Window { stream: 7, credit: 4096 });
        round(Msg::Close { stream: 7 });
        round(Msg::Reset { stream: 7 });
        round(Msg::Ping(42));
        round(Msg::Pong(42));
        round(Msg::Notice("monthly cap reached"));
        round(Msg::Goodbye { reconnect: true, reason: "edge restarting" });
        round(Msg::Challenge { nonce: [7; attest::NONCE_LEN] });
        round(Msg::Attest { key: "r1", mac: [9; attest::MAC_LEN] });
        round(Msg::Attest { key: "", mac: [0; attest::MAC_LEN] });
    }

    /// A `Hello` as clients before `features` encoded it.
    fn old_hello() -> Vec<u8> {
        let mut buf = Vec::new();
        frame(&mut buf, kind::HELLO, |o| {
            o.extend_from_slice(&PROTOCOL.to_le_bytes());
            for s in ["tok", "dev", "web", "localhost:3000", "kiri 0.0.9"] {
                put_str(o, s);
            }
        });
        buf
    }

    #[test]
    fn old_hello_has_no_features() {
        let old = old_hello();
        let Msg::Hello(h) = decode(&old).unwrap() else { panic!("not a Hello") };
        assert_eq!((h.client, h.features, h.machine, h.host), ("kiri 0.0.9", 0, "", ""));
    }

    #[test]
    fn a_hello_with_features_but_no_machine_reads_as_empty() {
        // What clients between `features` and `machine` send.
        let mut old = old_hello();
        put_u32(&mut old, FEATURE_ATTEST);
        let len = (old.len() - 4) as u32;
        old[..4].copy_from_slice(&len.to_le_bytes());
        let Msg::Hello(h) = decode(&old).unwrap() else { panic!("not a Hello") };
        assert_eq!((h.features, h.machine, h.host), (FEATURE_ATTEST, "", ""));
        // A newer client's Hello starts with exactly that, so those edges
        // read it the same and ignore the rest.
        let mut new = Vec::new();
        Msg::Hello(Hello { protocol: PROTOCOL, token: "tok", device: "dev", name: "web", target: "localhost:3000", client: "kiri 0.0.9", features: FEATURE_ATTEST, machine: "m1", host: "paul-mbp" }).encode(&mut new);
        assert_eq!(&new[4..old.len()], &old[4..]);
        assert_eq!(new.len(), old.len() + 4 + 2 + 4 + 8);
    }

    #[test]
    fn old_decoders_skip_the_features_field() {
        // Older edges decode a Hello field by field and ignore what follows,
        // so a new client's Hello reads the same to them minus `features`.
        let mut new = Vec::new();
        Msg::Hello(Hello { protocol: PROTOCOL, token: "tok", device: "dev", name: "web", target: "localhost:3000", client: "kiri 0.0.9", features: FEATURE_ATTEST, machine: "", host: "" }).encode(&mut new);
        let old = old_hello();
        assert_eq!(&new[4..old.len()], &old[4..]);
        assert_eq!(new.len(), old.len() + 4 + 4 + 4);
    }

    #[test]
    fn welcome_domains_are_optional_both_ways() {
        // An older edge's Welcome has no domains field.
        let mut old = Vec::new();
        frame(&mut old, kind::WELCOME, |o| {
            put_str(o, "https://web.runkiri.cloud");
            put_u32(o, 0);
            o.extend_from_slice(&u64::MAX.to_le_bytes());
            o.push(0);
        });
        let Msg::Welcome(w) = decode(&old).unwrap() else { panic!("not a Welcome") };
        assert_eq!((w.url, w.domains), ("https://web.runkiri.cloud", ""));
        // A newer edge's Welcome starts with exactly what older clients read.
        let mut new = Vec::new();
        Msg::Welcome(Welcome { url: "https://web.runkiri.cloud", rate: 0, remaining: u64::MAX, warning: false, domains: "https://dev.example.com", lanes: 2, lane_key: "k", window: 0 }).encode(&mut new);
        assert_eq!(&new[6..old.len()], &old[6..]);
        // And without lanes, read as none.
        let Msg::Welcome(w) = decode(&old).unwrap() else { panic!("not a Welcome") };
        assert_eq!((w.lanes, w.lane_key, w.send_window()), (0, "", INITIAL_WINDOW));
    }

    #[test]
    fn chunk_header_matches_encode() {
        let mut buf = Vec::new();
        Msg::Chunk { stream: 3, seq: 4, fin: true, data: b"hello" }.encode(&mut buf);
        assert_eq!(&buf[..CHUNK_HEADER], &chunk_header(3, 4, true, 5));
    }

    #[test]
    fn challenge_and_attest_reject_short_payloads() {
        let mut buf = Vec::new();
        Msg::Challenge { nonce: [1; attest::NONCE_LEN] }.encode(&mut buf);
        buf.pop();
        let len = (buf.len() - 4) as u32;
        buf[..4].copy_from_slice(&len.to_le_bytes());
        assert_eq!(decode(&buf), Err(DecodeError::Truncated));
        let mut buf = Vec::new();
        Msg::Attest { key: "r1", mac: [1; attest::MAC_LEN] }.encode(&mut buf);
        buf.truncate(buf.len() - 1);
        assert_eq!(decode(&buf), Err(DecodeError::Truncated));
    }

    #[test]
    fn data_header_matches_encode() {
        let mut buf = Vec::new();
        Msg::Data { stream: 3, data: b"hello" }.encode(&mut buf);
        assert_eq!(&buf[..HEADER + 4], &data_header(3, 5));
    }

    #[test]
    fn rejects_oversized_unknown_and_truncated() {
        assert_eq!(frame_len(&(MAX_FRAME as u32).to_le_bytes()), Err(DecodeError::TooLarge(MAX_FRAME)));
        assert_eq!(frame_len(&1u32.to_le_bytes()), Err(DecodeError::TooLarge(1)));
        assert_eq!(decode(&[2, 0, 0, 0, 99, 0]), Err(DecodeError::UnknownKind(99)));
        assert_eq!(decode(&[6, 0, 0, 0, 6, 0, 1, 0]), Err(DecodeError::Truncated));
        assert_eq!(decode(&[1, 0]), Err(DecodeError::Truncated));
    }
}
