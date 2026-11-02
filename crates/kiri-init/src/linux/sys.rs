//! Thin wrappers over the syscalls the guest uses. Everything unsafe lives
//! here or right next to the one call that needs it.

use std::ffi::{CStr, CString};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub fn boottime_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid, writable timespec.
    unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

pub fn check(ret: libc::c_long, what: &str) -> io::Result<libc::c_long> {
    if ret < 0 {
        let e = io::Error::last_os_error();
        return Err(io::Error::new(e.kind(), format!("{what}: {e}")));
    }
    Ok(ret)
}

pub fn cstr(p: impl AsRef<Path>) -> io::Result<CString> {
    CString::new(p.as_ref().as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
}

pub fn mount(src: &CStr, target: &CStr, fstype: &CStr, flags: libc::c_ulong, data: Option<&CStr>) -> io::Result<()> {
    // SAFETY: every pointer is a valid NUL-terminated string or null.
    let ret = unsafe {
        libc::mount(src.as_ptr(), target.as_ptr(), fstype.as_ptr(), flags, data.map_or(std::ptr::null(), |d| d.as_ptr().cast()))
    };
    check(ret as _, &format!("mount {}", target.to_string_lossy())).map(drop)
}

/// Mounts with no source or type, for changing propagation (`MS_PRIVATE` …).
pub fn remount(target: &CStr, flags: libc::c_ulong) -> io::Result<()> {
    // SAFETY: `target` is a valid NUL-terminated string; the rest may be null.
    let ret = unsafe { libc::mount(std::ptr::null(), target.as_ptr(), std::ptr::null(), flags, std::ptr::null()) };
    check(ret as _, &format!("remount {}", target.to_string_lossy())).map(drop)
}

pub fn umount_lazy(target: &Path) -> io::Result<()> {
    let t = cstr(target)?;
    // SAFETY: `t` is a valid NUL-terminated string.
    check(unsafe { libc::umount2(t.as_ptr(), libc::MNT_DETACH) } as _, "umount").map(drop)
}

pub fn mkdir(p: &CStr, mode: libc::mode_t) {
    // SAFETY: `p` is a valid NUL-terminated string. EEXIST is expected and fine.
    unsafe { libc::mkdir(p.as_ptr(), mode) };
}

pub fn mknod(p: &CStr, mode: libc::mode_t, major: u32, minor: u32) -> io::Result<()> {
    // SAFETY: `p` is a valid NUL-terminated string.
    let ret = unsafe { libc::mknod(p.as_ptr(), mode, libc::makedev(major, minor)) };
    check(ret as _, &format!("mknod {}", p.to_string_lossy())).map(drop)
}

pub fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as RawFd; 2];
    // SAFETY: `fds` has room for the two descriptors pipe2 writes.
    check(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } as _, "pipe2")?;
    // SAFETY: pipe2 succeeded, so both descriptors are fresh and ours.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

pub fn write_file(p: &str, contents: &str) -> io::Result<()> {
    std::fs::write(p, contents).map_err(|e| io::Error::new(e.kind(), format!("write {p}: {e}")))
}

/// Brings the loopback interface up in the current network namespace.
pub fn loopback_up() -> io::Result<()> {
    // SAFETY: plain socket creation; the fd is owned immediately.
    let fd = check(unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) } as _, "socket")?;
    // SAFETY: `fd` is a fresh, valid descriptor that nothing else owns.
    let sock = unsafe { OwnedFd::from_raw_fd(fd as RawFd) };
    // SAFETY: ifreq is plain data; all-zero is a valid value.
    let mut req: libc::ifreq = unsafe { std::mem::zeroed() };
    for (dst, src) in req.ifr_name.iter_mut().zip(b"lo\0") {
        *dst = *src as libc::c_char;
    }
    req.ifr_ifru.ifru_flags = (libc::IFF_UP | libc::IFF_RUNNING) as libc::c_short;
    use std::os::fd::AsRawFd;
    // SAFETY: `req` is a valid ifreq for SIOCSIFFLAGS.
    check(unsafe { libc::ioctl(sock.as_raw_fd(), libc::SIOCSIFFLAGS as libc::Ioctl, &req) } as _, "lo up").map(drop)
}

pub fn sethostname(name: &[u8]) -> io::Result<()> {
    // SAFETY: `name` is valid for `name.len()` bytes.
    check(unsafe { libc::sethostname(name.as_ptr().cast(), name.len()) } as _, "sethostname").map(drop)
}

pub fn setdomainname(name: &[u8]) -> io::Result<()> {
    // SAFETY: `name` is valid for `name.len()` bytes.
    check(unsafe { libc::setdomainname(name.as_ptr().cast(), name.len()) } as _, "setdomainname").map(drop)
}

/// Arguments for clone3(2). libc has no definition for it on musl yet.
#[repr(C)]
#[derive(Default)]
pub struct CloneArgs {
    pub flags: u64,
    pub pidfd: u64,
    pub child_tid: u64,
    pub parent_tid: u64,
    pub exit_signal: u64,
    pub stack: u64,
    pub stack_size: u64,
    pub tls: u64,
    pub set_tid: u64,
    pub set_tid_size: u64,
    pub cgroup: u64,
}

pub const CLONE_INTO_CGROUP: u64 = 0x2_0000_0000;

/// Like fork(), but straight into new namespaces and a cgroup. Returns 0 in
/// the child and the child's pid in the parent.
///
/// # Safety
/// The caller must be single-threaded, as with fork(): the child gets a copy
/// of this thread only, so no other thread may hold a lock it needs.
pub unsafe fn clone3(args: &CloneArgs) -> io::Result<libc::pid_t> {
    // SAFETY: `args` is a valid clone_args of the size passed; the caller
    // upholds the single-thread requirement.
    let ret = unsafe { libc::syscall(libc::SYS_clone3, args as *const CloneArgs, std::mem::size_of::<CloneArgs>()) };
    check(ret, "clone3").map(|p| p as libc::pid_t)
}

/// Writes `msg` straight to `fd` and exits without unwinding or running any
/// destructors. Only for the child between clone and exec.
pub fn child_fail(fd: RawFd, what: &str, err: io::Error) -> ! {
    let msg = format!("kiri: cannot start container: {what}: {err}\n");
    // SAFETY: writing a valid buffer to a descriptor we hold, then exiting.
    unsafe {
        libc::write(fd, msg.as_ptr().cast(), msg.len());
        libc::_exit(127)
    }
}

pub fn socketpair() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as RawFd; 2];
    // SAFETY: `fds` has room for the two descriptors socketpair writes.
    check(unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM | libc::SOCK_CLOEXEC, 0, fds.as_mut_ptr()) } as _, "socketpair")?;
    // SAFETY: socketpair succeeded, so both descriptors are fresh and ours.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

/// Sends one descriptor over a Unix socket (SCM_RIGHTS).
pub fn send_fd(sock: RawFd, fd: RawFd) -> io::Result<()> {
    let mut byte = [0u8; 1];
    let mut iov = libc::iovec { iov_base: byte.as_mut_ptr().cast(), iov_len: 1 };
    // SAFETY: CMSG_SPACE is a pure size computation.
    let space = unsafe { libc::CMSG_SPACE(std::mem::size_of::<RawFd>() as u32) } as usize;
    let mut control = vec![0u8; space];
    // SAFETY: msghdr is plain data; all-zero is valid before filling it in.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = space as _;
    // SAFETY: `msg` points at a control buffer big enough for one fd.
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<RawFd>() as u32) as _;
        std::ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>(), fd);
        check(libc::sendmsg(sock, &msg, 0) as _, "sendmsg").map(drop)
    }
}

/// What came over a socket: a descriptor, or the bytes of an error message.
pub enum Received {
    Fd(OwnedFd),
    Message(String),
}

/// Receives one descriptor, or whatever text the peer sent instead.
pub fn recv_fd(sock: RawFd) -> io::Result<Received> {
    let mut buf = [0u8; 512];
    let mut iov = libc::iovec { iov_base: buf.as_mut_ptr().cast(), iov_len: buf.len() };
    // SAFETY: CMSG_SPACE is a pure size computation.
    let space = unsafe { libc::CMSG_SPACE(std::mem::size_of::<RawFd>() as u32) } as usize;
    let mut control = vec![0u8; space];
    // SAFETY: msghdr is plain data; all-zero is valid before filling it in.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = space as _;
    // SAFETY: `msg` describes buffers we own; MSG_CMSG_CLOEXEC marks the fd.
    let n = check(unsafe { libc::recvmsg(sock, &mut msg, libc::MSG_CMSG_CLOEXEC) } as _, "recvmsg")?;
    // SAFETY: walking the control messages recvmsg filled in.
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        if !cmsg.is_null() && (*cmsg).cmsg_level == libc::SOL_SOCKET && (*cmsg).cmsg_type == libc::SCM_RIGHTS {
            let fd = std::ptr::read_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>());
            return Ok(Received::Fd(OwnedFd::from_raw_fd(fd)));
        }
    }
    Ok(Received::Message(String::from_utf8_lossy(&buf[..n as usize]).trim().to_owned()))
}

pub fn setns(fd: RawFd, nstype: libc::c_int) -> io::Result<()> {
    // SAFETY: plain syscall on a namespace descriptor we hold.
    check(unsafe { libc::setns(fd, nstype) } as _, "setns").map(drop)
}

pub fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: plain fcntl calls on a descriptor we own.
    unsafe {
        let flags = check(libc::fcntl(fd, libc::F_GETFL) as _, "fcntl")?;
        check(libc::fcntl(fd, libc::F_SETFL, flags as libc::c_int | libc::O_NONBLOCK) as _, "fcntl").map(drop)
    }
}

/// Opens a new pseudo-terminal pair from the current mount namespace's
/// /dev/ptmx. Returns (master, slave).
pub fn open_pty() -> io::Result<(OwnedFd, OwnedFd)> {
    // SAFETY: plain open/ioctl calls; descriptors are owned right away.
    unsafe {
        let master = check(libc::open(c"/dev/ptmx".as_ptr(), libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC) as _, "open /dev/ptmx")?;
        let master = OwnedFd::from_raw_fd(master as RawFd);
        let unlock: libc::c_int = 0;
        check(libc::ioctl(master.as_raw_fd(), libc::TIOCSPTLCK as libc::Ioctl, &unlock) as _, "unlock pty")?;
        let mut n: libc::c_uint = 0;
        check(libc::ioctl(master.as_raw_fd(), libc::TIOCGPTN as libc::Ioctl, &mut n) as _, "pty number")?;
        let path = CString::new(format!("/dev/pts/{n}")).expect("no NUL");
        let slave = check(libc::open(path.as_ptr(), libc::O_RDWR | libc::O_NOCTTY) as _, "open pty slave")?;
        Ok((master, OwnedFd::from_raw_fd(slave as RawFd)))
    }
}

pub fn resize_pty(fd: RawFd, rows: u16, cols: u16) {
    let ws = libc::winsize { ws_row: rows, ws_col: cols, ws_xpixel: 0, ws_ypixel: 0 };
    // SAFETY: TIOCSWINSZ with a valid winsize on a descriptor we own.
    unsafe { libc::ioctl(fd, libc::TIOCSWINSZ as libc::Ioctl, &ws) };
}
