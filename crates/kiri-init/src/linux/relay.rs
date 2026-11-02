//! Port forwarding, guest side: the host opens a vsock stream per client
//! connection, sends `ip[4] port[2]`, and the agent relays bytes between it
//! and a TCP connection to that address. Non-blocking, from the agent's poll
//! loop, with bounded buffers in each direction.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use super::sys;

/// vsock port the agent accepts forwarded connections on.
pub const PORT: u32 = 1025;
/// Forwarded UDP: the same header, then datagrams framed as `u16 len | bytes`
/// both ways on one stream per client address.
pub const UDP_PORT: u32 = 1029;
/// Forwarded TCP as on `PORT`, but what the container sends comes back
/// framed as `u32 len | bytes`, and an empty frame is its end. The Mac
/// side learns the end from that frame, not from the stream's half-close:
/// Virtualization.framework drops a guest's vsock half-close that follows
/// data too closely (a few in a thousand under concurrent connections), and
/// the client then waits for an end that never comes.
pub const FRAMED_PORT: u32 = 1030;
/// Largest UDP payload.
const MAX_DATAGRAM: usize = 65_507;
const BUF: usize = 64 * 1024;

pub fn listen() -> io::Result<OwnedFd> {
    listen_vsock(PORT)
}

/// A non-blocking listener on `port` (vsock, or a unix socket on a Linux host).
pub fn listen_vsock(port: u32) -> io::Result<OwnedFd> {
    super::transport::listen(port)
}

/// One direction of a relay.
struct Pipe {
    buf: Vec<u8>,
    /// The reading side reached EOF; once `buf` drains, shut the writer.
    eof: bool,
    shut: bool,
}

impl Pipe {
    fn new() -> Pipe {
        Pipe { buf: Vec::with_capacity(BUF), eof: false, shut: false }
    }
}

pub struct Relay {
    host: OwnedFd,
    target: Option<OwnedFd>,
    connected: bool,
    header: Vec<u8>,
    /// host → container, container → host.
    up: Pipe,
    down: Pipe,
    pub dead: bool,
    udp: bool,
    /// The container's bytes go to the host in frames (`FRAMED_PORT`).
    framed: bool,
}

pub fn listen_udp() -> io::Result<OwnedFd> {
    listen_vsock(UDP_PORT)
}

/// A forwarded UDP client: datagrams to and from `ip:port` in a container.
pub fn accept_udp(listener: &OwnedFd) -> Option<Relay> {
    accept(listener).map(|r| Relay { udp: true, ..r })
}

pub fn listen_framed() -> io::Result<OwnedFd> {
    listen_vsock(FRAMED_PORT)
}

/// A forwarded TCP client whose replies go back framed.
pub fn accept_framed(listener: &OwnedFd) -> Option<Relay> {
    accept(listener).map(|r| Relay { framed: true, ..r })
}

pub fn accept(listener: &OwnedFd) -> Option<Relay> {
    // SAFETY: accept4 on a listening socket we own; the fd is owned immediately.
    let fd = unsafe { libc::accept4(listener.as_raw_fd(), std::ptr::null_mut(), std::ptr::null_mut(), libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    // SAFETY: `fd` is a fresh descriptor that nothing else owns.
    let host = unsafe { OwnedFd::from_raw_fd(fd) };
    Some(Relay { host, target: None, connected: false, header: Vec::with_capacity(6), up: Pipe::new(), down: Pipe::new(), dead: false, udp: false, framed: false })
}

fn read_into(fd: RawFd, buf: &mut Vec<u8>, max: usize) -> io::Result<usize> {
    let start = buf.len();
    let room = max - start;
    if room == 0 {
        return Ok(1); // not EOF; just full
    }
    buf.resize(start + room, 0);
    // SAFETY: reading into the spare part of a buffer we own.
    let n = unsafe { libc::read(fd, buf[start..].as_mut_ptr().cast(), room) };
    if n < 0 {
        buf.truncate(start);
        return Err(io::Error::last_os_error());
    }
    buf.truncate(start + n as usize);
    Ok(n as usize)
}

/// `read_into` as one frame for the host: `u32 len | bytes`. EOF adds the
/// empty frame that ends the stream.
fn read_frame_into(fd: RawFd, buf: &mut Vec<u8>, max: usize) -> io::Result<usize> {
    let start = buf.len();
    if max <= start + 4 {
        return Ok(1); // not EOF; just full
    }
    buf.resize(max, 0);
    // SAFETY: reading into the spare part of a buffer we own, after the frame's length.
    let n = unsafe { libc::read(fd, buf[start + 4..].as_mut_ptr().cast(), max - start - 4) };
    if n < 0 {
        buf.truncate(start);
        return Err(io::Error::last_os_error());
    }
    buf[start..start + 4].copy_from_slice(&(n as u32).to_be_bytes());
    buf.truncate(start + 4 + n as usize);
    Ok(n as usize)
}

fn write_from(fd: RawFd, buf: &mut Vec<u8>) -> io::Result<()> {
    if buf.is_empty() {
        return Ok(());
    }
    // SAFETY: writing from a buffer we own.
    let n = unsafe { libc::write(fd, buf.as_ptr().cast(), buf.len()) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    buf.drain(..n as usize);
    Ok(())
}

fn would_block(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(libc::EAGAIN) | Some(libc::EINPROGRESS))
}

impl Relay {
    /// (fd, events) pairs to poll for this relay.
    pub fn interest(&self, out: &mut Vec<libc::pollfd>) {
        let host_ev = if self.target.is_none() || (!self.up.eof && self.up.buf.len() < BUF) { libc::POLLIN } else { 0 }
            | if self.down.buf.is_empty() { 0 } else { libc::POLLOUT };
        out.push(libc::pollfd { fd: self.host.as_raw_fd(), events: host_ev, revents: 0 });
        if let (Some(t), true) = (&self.target, self.udp) {
            out.push(libc::pollfd { fd: t.as_raw_fd(), events: libc::POLLIN, revents: 0 });
            return;
        }
        if let Some(t) = &self.target {
            let ev = if !self.connected {
                libc::POLLOUT
            } else {
                (if !self.down.eof && self.down.buf.len() < BUF { libc::POLLIN } else { 0 })
                    | if self.up.buf.is_empty() { 0 } else { libc::POLLOUT }
            };
            out.push(libc::pollfd { fd: t.as_raw_fd(), events: ev, revents: 0 });
        }
    }

    pub fn owns(&self, fd: RawFd) -> bool {
        self.host.as_raw_fd() == fd || self.target.as_ref().is_some_and(|t| t.as_raw_fd() == fd)
    }

    /// Makes progress after poll; marks the relay dead when it is finished.
    pub fn step(&mut self) {
        if let Err(e) = self.try_step() {
            if !would_block(&e) {
                self.dead = true;
            }
        }
        if self.up.shut && self.down.shut {
            self.dead = true;
        }
    }

    fn try_step(&mut self) -> io::Result<()> {
        if self.target.is_none() {
            // The header: where to connect.
            let need = 6 - self.header.len();
            let mut b = [0u8; 6];
            // SAFETY: reading into a local buffer.
            let n = unsafe { libc::read(self.host.as_raw_fd(), b.as_mut_ptr().cast(), need) };
            if n == 0 {
                self.dead = true;
                return Ok(());
            }
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            self.header.extend_from_slice(&b[..n as usize]);
            if self.header.len() < 6 {
                return Ok(());
            }
            let ip: [u8; 4] = self.header[..4].try_into().expect("4");
            let port = u16::from_be_bytes([self.header[4], self.header[5]]);
            if self.udp {
                self.target = Some(connect_udp(ip, port)?);
                self.connected = true;
            } else {
                self.target = Some(connect(ip, port)?);
            }
            return Ok(());
        }
        if self.udp {
            let target = self.target.as_ref().expect("checked").as_raw_fd();
            return udp_step(self.host.as_raw_fd(), target, &mut self.up, &mut self.down);
        }
        let target = self.target.as_ref().expect("checked").as_raw_fd();
        if !self.connected {
            let mut err: libc::c_int = 0;
            let mut len = std::mem::size_of::<libc::c_int>() as u32;
            // SAFETY: reading SO_ERROR into a local int.
            unsafe { libc::getsockopt(target, libc::SOL_SOCKET, libc::SO_ERROR, (&mut err as *mut libc::c_int).cast(), &mut len) };
            if err == libc::EINPROGRESS || err == libc::EALREADY {
                return Ok(());
            }
            if err != 0 {
                self.dead = true; // refused: the client sees its connection close
                return Ok(());
            }
            // Writable without error means connected; one quick probe tells
            // a still-pending connect apart.
            // SAFETY: plain getpeername into a local buffer.
            let mut sa: libc::sockaddr_in = unsafe { std::mem::zeroed() };
            let mut sl = std::mem::size_of::<libc::sockaddr_in>() as u32;
            if unsafe { libc::getpeername(target, (&mut sa as *mut libc::sockaddr_in).cast(), &mut sl) } != 0 {
                return Ok(());
            }
            self.connected = true;
        }
        let host = self.host.as_raw_fd();
        pump(host, target, &mut self.up, false)?;
        pump(target, host, &mut self.down, self.framed)?;
        Ok(())
    }
}

/// Moves bytes from `from` to `to` through `pipe`, passing EOF on as a
/// half-close once everything read has been written.
fn pump(from: RawFd, to: RawFd, pipe: &mut Pipe, framed: bool) -> io::Result<()> {
    if !pipe.eof && pipe.buf.len() < BUF {
        let read = if framed { read_frame_into(from, &mut pipe.buf, BUF) } else { read_into(from, &mut pipe.buf, BUF) };
        match read {
            Ok(0) => pipe.eof = true,
            Ok(_) => {}
            Err(e) if would_block(&e) => {}
            Err(e) => return Err(e),
        }
    }
    match write_from(to, &mut pipe.buf) {
        Ok(()) => {}
        Err(e) if would_block(&e) => {}
        Err(e) => return Err(e),
    }
    if pipe.eof && pipe.buf.is_empty() && !pipe.shut {
        // SAFETY: half-closing a socket we own.
        unsafe { libc::shutdown(to, libc::SHUT_WR) };
        pipe.shut = true;
    }
    Ok(())
}

/// Frames from the host become datagrams; datagrams become frames. A full
/// socket buffer drops datagrams, as UDP may.
fn udp_step(host: RawFd, target: RawFd, up: &mut Pipe, down: &mut Pipe) -> io::Result<()> {
    if !up.eof && up.buf.len() < BUF {
        match read_into(host, &mut up.buf, BUF) {
            Ok(0) => up.eof = true,
            Ok(_) => {}
            Err(e) if would_block(&e) => {}
            Err(e) => return Err(e),
        }
    }
    while up.buf.len() >= 2 {
        let len = u16::from_be_bytes([up.buf[0], up.buf[1]]) as usize;
        if up.buf.len() < 2 + len {
            break;
        }
        // SAFETY: sending from a buffer we own on a connected socket.
        unsafe { libc::send(target, up.buf[2..].as_ptr().cast(), len, libc::MSG_DONTWAIT) };
        up.buf.drain(..2 + len);
    }
    if up.eof {
        // The host ended the session (idle, or the client went away).
        up.shut = true;
        down.shut = true;
        return Ok(());
    }
    let mut d = [0u8; MAX_DATAGRAM];
    while down.buf.len() + 2 + MAX_DATAGRAM <= BUF * 2 {
        // SAFETY: receiving into a local buffer.
        let n = unsafe { libc::recv(target, d.as_mut_ptr().cast(), d.len(), libc::MSG_DONTWAIT) };
        if n < 0 {
            break; // EAGAIN, or ECONNREFUSED from an ICMP: nothing to pass on
        }
        down.buf.extend_from_slice(&(n as u16).to_be_bytes());
        down.buf.extend_from_slice(&d[..n as usize]);
    }
    match write_from(host, &mut down.buf) {
        Ok(()) => Ok(()),
        Err(e) if would_block(&e) => Ok(()),
        Err(e) => Err(e),
    }
}

fn connect_udp(ip: [u8; 4], port: u16) -> io::Result<OwnedFd> {
    // SAFETY: plain socket setup; the fd is owned immediately.
    unsafe {
        let fd = sys::check(libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK, 0) as _, "socket")?;
        let sock = OwnedFd::from_raw_fd(fd as RawFd);
        let mut a: libc::sockaddr_in = std::mem::zeroed();
        a.sin_family = libc::AF_INET as libc::sa_family_t;
        a.sin_port = port.to_be();
        a.sin_addr.s_addr = u32::from_ne_bytes(ip);
        sys::check(libc::connect(sock.as_raw_fd(), (&a as *const libc::sockaddr_in).cast(), std::mem::size_of::<libc::sockaddr_in>() as u32) as _, "connect")?;
        Ok(sock)
    }
}

fn connect(ip: [u8; 4], port: u16) -> io::Result<OwnedFd> {
    // SAFETY: plain non-blocking connect; the fd is owned immediately.
    unsafe {
        let fd = sys::check(libc::socket(libc::AF_INET, libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK, 0) as _, "socket")?;
        let sock = OwnedFd::from_raw_fd(fd as RawFd);
        let one: libc::c_int = 1;
        libc::setsockopt(sock.as_raw_fd(), libc::IPPROTO_TCP, libc::TCP_NODELAY, (&one as *const libc::c_int).cast(), 4);
        let mut a: libc::sockaddr_in = std::mem::zeroed();
        a.sin_family = libc::AF_INET as libc::sa_family_t;
        a.sin_port = port.to_be();
        a.sin_addr.s_addr = u32::from_ne_bytes(ip);
        let r = libc::connect(sock.as_raw_fd(), (&a as *const libc::sockaddr_in).cast(), std::mem::size_of::<libc::sockaddr_in>() as u32);
        if r != 0 {
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(libc::EINPROGRESS) {
                return Err(e);
            }
        }
        Ok(sock)
    }
}

/// The reverse direction, for `host.docker.internal`: TCP connections a
/// container makes to the reserved host address are redirected here.
pub fn listen_host_bound(port: u16) -> io::Result<OwnedFd> {
    // SAFETY: plain socket setup; the fd is owned immediately.
    unsafe {
        let fd = sys::check(libc::socket(libc::AF_INET, libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK, 0) as _, "socket")?;
        let sock = OwnedFd::from_raw_fd(fd as RawFd);
        let one: libc::c_int = 1;
        libc::setsockopt(sock.as_raw_fd(), libc::SOL_SOCKET, libc::SO_REUSEADDR, (&one as *const libc::c_int).cast(), 4);
        let mut a: libc::sockaddr_in = std::mem::zeroed();
        a.sin_family = libc::AF_INET as libc::sa_family_t;
        a.sin_port = port.to_be();
        sys::check(libc::bind(sock.as_raw_fd(), (&a as *const libc::sockaddr_in).cast(), std::mem::size_of::<libc::sockaddr_in>() as u32) as _, "bind")?;
        sys::check(libc::listen(sock.as_raw_fd(), 128) as _, "listen")?;
        Ok(sock)
    }
}

/// The host side listens on this vsock port for host-bound connections.
pub const HOST_VSOCK_PORT: u32 = 1026;

/// Accepts one redirected connection, finds the port the container meant,
/// dials the Mac and returns a relay between the two. Only connections that
/// were redirected from `host_addr` are relayed: one made to the listener
/// directly (from the LAN, on a Linux host) would reach the host's loopback.
pub fn accept_host_bound(listener: &OwnedFd, host_addr: [u8; 4]) -> Option<Relay> {
    // SAFETY: accept4 on a listening socket we own; the fd is owned immediately.
    let fd = unsafe { libc::accept4(listener.as_raw_fd(), std::ptr::null_mut(), std::ptr::null_mut(), libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    // SAFETY: `fd` is a fresh descriptor that nothing else owns.
    let client = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: SO_ORIGINAL_DST fills a sockaddr_in with the pre-redirect destination.
    let port = unsafe {
        let mut dst: libc::sockaddr_in = std::mem::zeroed();
        let mut len = std::mem::size_of::<libc::sockaddr_in>() as u32;
        const SO_ORIGINAL_DST: libc::c_int = 80;
        if libc::getsockopt(client.as_raw_fd(), libc::SOL_IP, SO_ORIGINAL_DST, (&mut dst as *mut libc::sockaddr_in).cast(), &mut len) != 0 {
            return Some(Relay::dead_one(client));
        }
        if dst.sin_addr.s_addr != u32::from_ne_bytes(host_addr) {
            return Some(Relay::dead_one(client));
        }
        u16::from_be(dst.sin_port)
    };
    let Ok(host) = dial_host(HOST_VSOCK_PORT) else { return Some(Relay::dead_one(client)) };
    // The Mac connects to its own loopback: services bound to 127.0.0.1 work.
    let mut header = [127, 0, 0, 1, 0, 0];
    header[4..].copy_from_slice(&port.to_be_bytes());
    // SAFETY: a 6-byte write to a just-connected stream; the socket buffer is empty.
    if unsafe { libc::write(host.as_raw_fd(), header.as_ptr().cast(), 6) } != 6 {
        return Some(Relay::dead_one(client));
    }
    if sys::set_nonblocking(host.as_raw_fd()).is_err() {
        return Some(Relay::dead_one(client));
    }
    Some(Relay {
        host,
        target: Some(client),
        connected: true,
        header: Vec::new(),
        up: Pipe::new(),
        down: Pipe::new(),
        dead: false,
        udp: false,
        framed: false,
    })
}

fn dial_host(port: u32) -> io::Result<OwnedFd> {
    super::transport::dial(port)
}

impl Relay {
    /// A relay that only exists to close `fd` on the next sweep.
    fn dead_one(fd: OwnedFd) -> Relay {
        Relay { host: fd, target: None, connected: false, header: Vec::new(), up: Pipe::new(), down: Pipe::new(), dead: true, udp: false, framed: false }
    }
}

/// SSH agent forwarding: containers that mount this socket reach the Mac's
/// SSH agent. Each connection is relayed over vsock; keys never leave the Mac.
pub const SSH_AGENT_SOCK: &str = "/run/host-services/ssh-auth.sock";
pub const SSH_VSOCK_PORT: u32 = 1027;

pub fn listen_ssh_agent() -> io::Result<OwnedFd> {
    std::fs::create_dir_all("/run/host-services")?;
    let _ = std::fs::remove_file(SSH_AGENT_SOCK);
    // SAFETY: plain unix socket setup; the fd is owned immediately.
    unsafe {
        // The socket below is 0777 for any user in a container. On a Linux
        // host this directory is the host's own, so only root, which
        // bind-mounts the socket into containers, may reach it by path.
        let dir = std::ffi::CString::new("/run/host-services").expect("no NUL");
        sys::check(libc::chmod(dir.as_ptr(), 0o700) as _, "chmod /run/host-services")?;
        let fd = sys::check(libc::socket(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK, 0) as _, "socket")?;
        let sock = OwnedFd::from_raw_fd(fd as RawFd);
        let mut a: libc::sockaddr_un = std::mem::zeroed();
        a.sun_family = libc::AF_UNIX as libc::sa_family_t;
        for (d, s) in a.sun_path.iter_mut().zip(SSH_AGENT_SOCK.bytes()) {
            *d = s as libc::c_char;
        }
        sys::check(libc::bind(sock.as_raw_fd(), (&a as *const libc::sockaddr_un).cast(), std::mem::size_of::<libc::sockaddr_un>() as u32) as _, "bind")?;
        // Any user in a container may use it, as with Docker Desktop.
        let path = std::ffi::CString::new(SSH_AGENT_SOCK).expect("no NUL");
        libc::chmod(path.as_ptr(), 0o777);
        sys::check(libc::listen(sock.as_raw_fd(), 64) as _, "listen")?;
        Ok(sock)
    }
}

pub fn accept_ssh_agent(listener: &OwnedFd) -> Option<Relay> {
    // SAFETY: accept4 on a listening socket we own; the fd is owned immediately.
    let fd = unsafe { libc::accept4(listener.as_raw_fd(), std::ptr::null_mut(), std::ptr::null_mut(), libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    // SAFETY: fresh descriptor.
    let client = unsafe { OwnedFd::from_raw_fd(fd) };
    let Ok(host) = dial_host(SSH_VSOCK_PORT) else { return Some(Relay::dead_one(client)) };
    if sys::set_nonblocking(host.as_raw_fd()).is_err() {
        return Some(Relay::dead_one(client));
    }
    Some(Relay { host, target: Some(client), connected: true, header: Vec::new(), up: Pipe::new(), down: Pipe::new(), dead: false, udp: false, framed: false })
}

/// The engine's own API, for containers that mount the docker socket
/// (Testcontainers' Ryuk, CI runners): crates/kiri/src/daemon/binds.rs
/// mounts this socket for `/var/run/docker.sock`, and each connection is
/// relayed over vsock to the daemon on the Mac. In the VM only: on a Linux
/// host containers mount the daemon's socket itself.
pub const ENGINE_SOCK: &str = "/run/host-services/docker.sock";
pub const ENGINE_VSOCK_PORT: u32 = 1036;

pub fn listen_engine() -> io::Result<OwnedFd> {
    std::fs::create_dir_all("/run/host-services")?;
    let _ = std::fs::remove_file(ENGINE_SOCK);
    // SAFETY: plain unix socket setup; the fd is owned immediately.
    unsafe {
        let fd = sys::check(libc::socket(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK, 0) as _, "socket")?;
        let sock = OwnedFd::from_raw_fd(fd as RawFd);
        let mut a: libc::sockaddr_un = std::mem::zeroed();
        a.sun_family = libc::AF_UNIX as libc::sa_family_t;
        for (d, s) in a.sun_path.iter_mut().zip(ENGINE_SOCK.bytes()) {
            *d = s as libc::c_char;
        }
        sys::check(libc::bind(sock.as_raw_fd(), (&a as *const libc::sockaddr_un).cast(), std::mem::size_of::<libc::sockaddr_un>() as u32) as _, "bind")?;
        // root in a container that was given it, as Docker's root:docker 0660.
        let path = std::ffi::CString::new(ENGINE_SOCK).expect("no NUL");
        libc::chmod(path.as_ptr(), 0o660);
        sys::check(libc::listen(sock.as_raw_fd(), 64) as _, "listen")?;
        Ok(sock)
    }
}

pub fn accept_engine(listener: &OwnedFd) -> Option<Relay> {
    // SAFETY: accept4 on a listening socket we own; the fd is owned immediately.
    let fd = unsafe { libc::accept4(listener.as_raw_fd(), std::ptr::null_mut(), std::ptr::null_mut(), libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    // SAFETY: fresh descriptor.
    let client = unsafe { OwnedFd::from_raw_fd(fd) };
    let Ok(host) = dial_host(ENGINE_VSOCK_PORT) else { return Some(Relay::dead_one(client)) };
    if sys::set_nonblocking(host.as_raw_fd()).is_err() {
        return Some(Relay::dead_one(client));
    }
    Some(Relay { host, target: Some(client), connected: true, header: Vec::new(), up: Pipe::new(), down: Pipe::new(), dead: false, udp: false, framed: false })
}

#[cfg(test)]
mod tests {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    #[test]
    fn framed_reads_carry_their_length_and_end_with_an_empty_frame() {
        let mut fds = [0; 2];
        // SAFETY: `fds` has room for the two descriptors pipe writes.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        // SAFETY: pipe succeeded, so both are fresh and ours.
        let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        // SAFETY: writing a local buffer to a pipe we own.
        assert_eq!(unsafe { libc::write(w.as_raw_fd(), b"abc".as_ptr().cast(), 3) }, 3);
        drop(w);
        let mut buf = Vec::new();
        assert_eq!(super::read_frame_into(r.as_raw_fd(), &mut buf, super::BUF).unwrap(), 3);
        assert_eq!(super::read_frame_into(r.as_raw_fd(), &mut buf, super::BUF).unwrap(), 0);
        assert_eq!(buf, [0, 0, 0, 3, b'a', b'b', b'c', 0, 0, 0, 0]);
        // No room for a frame: not read, and not taken for EOF.
        let full = buf.len() + 4;
        assert_eq!(super::read_frame_into(r.as_raw_fd(), &mut buf, full).unwrap(), 1);
    }
}
