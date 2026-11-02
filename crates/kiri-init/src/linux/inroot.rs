//! Path lookup scoped to a container's root, as runc's securejoin does.
//!
//! The child mounts into the container before pivot_root, while its root is
//! still the VM's (or the Linux host's). A plain path walk there follows an
//! image's absolute symlink (`/data -> /mnt/mac/...`) out of the container,
//! so every mount target, device node and /etc file is looked up here
//! instead: one component at a time with O_NOFOLLOW, symlinks read and
//! resolved against the container root, `..` stopping at it. The result is
//! an O_PATH descriptor, used through `/proc/self/fd/N`.

use std::collections::VecDeque;
use std::ffi::{CString, OsStr, OsString};
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path};

/// Symlinks followed in one lookup before ELOOP, as Linux's limit.
const MAX_LINKS: u32 = 40;

/// What to make when the last component is missing. Missing directories on
/// the way are always made (mode 0755), as `create_dir_all` did.
#[derive(Clone, Copy)]
pub enum Last {
    /// It must exist.
    Exists,
    Dir,
    /// An empty file (0644).
    File,
    /// A device node: mode (type and permissions) and device number.
    Node(libc::mode_t, libc::dev_t),
}

/// A target found (or made) inside the root.
pub struct Found {
    pub fd: OwnedFd,
    /// The last component was missing and has just been made.
    pub created: bool,
}

impl Found {
    /// `/proc/self/fd/N`: a path the kernel resolves to exactly this object,
    /// for mount(2), chmod(2) and opening it for reading.
    pub fn proc_path(&self) -> CString {
        CString::new(format!("/proc/self/fd/{}", self.fd.as_raw_fd())).expect("no NUL")
    }
}

/// Opens the directory `root` (the container's rootfs) for lookups.
pub fn open_root(root: &Path) -> io::Result<OwnedFd> {
    let c = super::sys::cstr(root)?;
    // SAFETY: valid NUL-terminated path; the descriptor is owned right away.
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fresh descriptor nothing else owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Looks `path` up inside `root` the way the container will see it once
/// root is `/`: absolute symlinks and `..` never leave `root`.
pub fn open(root: BorrowedFd<'_>, path: &Path, last: Last) -> io::Result<Found> {
    let mut todo: VecDeque<OsString> = components(path.as_os_str());
    // Directories walked so far; [0] is the root, which `..` never pops.
    let mut dirs: Vec<OwnedFd> = vec![root.try_clone_to_owned()?];
    let mut links = 0;
    let mut created = false;
    while let Some(name) = todo.pop_front() {
        if name == ".." {
            if dirs.len() > 1 {
                dirs.pop();
            }
            continue;
        }
        let is_last = todo.is_empty();
        let dir = dirs.last().expect("root stays").as_fd();
        let fd = match open_nofollow(dir, &name) {
            Ok(fd) => fd,
            Err(e) if e.raw_os_error() == Some(libc::ENOENT) => {
                let kind = if is_last { last } else { Last::Dir };
                make(dir, &name, kind).map_err(|e| with_path(e, path))?;
                created = is_last;
                open_nofollow(dir, &name).map_err(|e| with_path(e, path))?
            }
            Err(e) => return Err(with_path(e, path)),
        };
        let st = fstat(fd.as_fd())?;
        match st.st_mode & libc::S_IFMT {
            libc::S_IFLNK => {
                links += 1;
                if links > MAX_LINKS {
                    return Err(with_path(io::Error::from_raw_os_error(libc::ELOOP), path));
                }
                let target = readlink(fd.as_fd())?;
                if target.as_bytes().first() == Some(&b'/') {
                    dirs.truncate(1);
                }
                for c in components(&target).into_iter().rev() {
                    todo.push_front(c);
                }
                // The link was made just now only if something raced us;
                // what it points at is looked up (and made) as usual.
                created = false;
            }
            libc::S_IFDIR if !is_last => dirs.push(fd),
            _ if is_last => return Ok(Found { fd, created }),
            _ => return Err(with_path(io::Error::from_raw_os_error(libc::ENOTDIR), path)),
        }
    }
    // The path was the root itself, or ended in `..`/a symlink to a directory.
    let fd = dirs.pop().expect("root stays");
    Ok(Found { fd, created })
}

/// The lexical form of what `open` reaches, for tests and messages.
#[cfg(test)]
fn components_of(path: &Path) -> Vec<OsString> {
    components(path.as_os_str()).into_iter().collect()
}

fn components(path: &OsStr) -> VecDeque<OsString> {
    Path::new(path)
        .components()
        .filter_map(|c| match c {
            Component::Normal(n) => Some(n.to_owned()),
            Component::ParentDir => Some(OsString::from("..")),
            Component::RootDir | Component::CurDir | Component::Prefix(_) => None,
        })
        .collect()
}

fn with_path(e: io::Error, path: &Path) -> io::Error {
    io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

fn name_c(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
}

fn open_nofollow(dir: BorrowedFd<'_>, name: &OsStr) -> io::Result<OwnedFd> {
    let c = name_c(name)?;
    // SAFETY: valid directory fd and NUL-terminated name; owned right away.
    let fd = unsafe { libc::openat(dir.as_raw_fd(), c.as_ptr(), libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fresh descriptor nothing else owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Makes `name` in `dir`; one that appeared meanwhile is fine.
fn make(dir: BorrowedFd<'_>, name: &OsStr, kind: Last) -> io::Result<()> {
    let c = name_c(name)?;
    let d = dir.as_raw_fd();
    // SAFETY: valid directory fd and NUL-terminated name.
    let ret = unsafe {
        match kind {
            Last::Exists => return Err(io::Error::from_raw_os_error(libc::ENOENT)),
            Last::Dir => libc::mkdirat(d, c.as_ptr(), 0o755),
            Last::File => {
                let fd = libc::openat(d, c.as_ptr(), libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC, 0o644);
                if fd >= 0 {
                    libc::close(fd);
                }
                fd
            }
            Last::Node(mode, dev) => libc::mknodat(d, c.as_ptr(), mode, dev),
        }
    };
    if ret < 0 {
        let e = io::Error::last_os_error();
        if e.raw_os_error() != Some(libc::EEXIST) {
            return Err(e);
        }
    }
    Ok(())
}

fn fstat(fd: BorrowedFd<'_>) -> io::Result<libc::stat> {
    // SAFETY: stat is plain data (all-zero is valid); fstat fills it from
    // a valid descriptor.
    unsafe {
        let mut st: libc::stat = std::mem::zeroed();
        if libc::fstat(fd.as_raw_fd(), &mut st) < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(st)
    }
}

/// The target of the symlink `fd` (opened O_PATH|O_NOFOLLOW).
fn readlink(fd: BorrowedFd<'_>) -> io::Result<OsString> {
    let mut buf = vec![0u8; libc::PATH_MAX as usize];
    // SAFETY: an empty name reads the link `fd` itself; `buf` is writable.
    let n = unsafe { libc::readlinkat(fd.as_raw_fd(), c"".as_ptr(), buf.as_mut_ptr().cast(), buf.len()) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    buf.truncate(n as usize);
    Ok(OsString::from_vec(buf))
}

/// Where `fd` is, as the kernel names it.
#[cfg(test)]
fn real_path(fd: BorrowedFd<'_>) -> io::Result<std::path::PathBuf> {
    std::fs::read_link(format!("/proc/self/fd/{}", fd.as_raw_fd()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    struct Tmp(std::path::PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A fake rootfs beside an "outside" directory standing in for /mnt/mac.
    fn setup(tag: &str) -> (Tmp, std::path::PathBuf, std::path::PathBuf) {
        let base = std::env::temp_dir().join(format!("kiri-inroot-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (root, outside) = (base.join("root"), base.join("outside"));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(outside.join("secret")).unwrap();
        fs::write(outside.join("secret/key.txt"), "mac").unwrap();
        (Tmp(base), root, outside)
    }

    fn rel(root: &Path, f: &Found) -> std::path::PathBuf {
        real_path(f.fd.as_fd()).unwrap().strip_prefix(root.canonicalize().unwrap()).unwrap().to_owned()
    }

    #[test]
    fn absolute_symlink_stays_in_root() {
        let (_t, root, outside) = setup("abs");
        symlink(outside.join("secret"), root.join("data")).unwrap();
        let r = open_root(&root).unwrap();
        let f = open(r.as_fd(), Path::new("/data"), Last::Dir).unwrap();
        // Made inside the root at the link's path, not the outside one.
        assert!(f.created);
        assert!(root.join(outside.strip_prefix("/").unwrap()).join("secret").is_dir());
        assert_eq!(rel(&root, &f), outside.strip_prefix("/").unwrap().join("secret"));
        assert_eq!(fs::read_dir(outside.join("secret")).unwrap().count(), 1);
    }

    #[test]
    fn dotdot_stops_at_root() {
        let (_t, root, outside) = setup("dotdot");
        symlink("../../../../../../../../outside", root.join("up")).unwrap();
        let r = open_root(&root).unwrap();
        let f = open(r.as_fd(), Path::new("up/x/y"), Last::File).unwrap();
        assert_eq!(rel(&root, &f), Path::new("outside/x/y"));
        assert!(!outside.join("x").exists());
        let f = open(r.as_fd(), Path::new("a/../../.."), Last::Dir).unwrap();
        assert_eq!(rel(&root, &f), Path::new(""));
    }

    #[test]
    fn in_image_symlink_is_followed() {
        // A legitimate link such as /var/run -> /run keeps working.
        let (_t, root, _o) = setup("legit");
        fs::create_dir_all(root.join("run")).unwrap();
        fs::create_dir_all(root.join("var")).unwrap();
        symlink("/run", root.join("var/run")).unwrap();
        symlink("../run/app.sock", root.join("var/sock")).unwrap();
        let r = open_root(&root).unwrap();
        let f = open(r.as_fd(), Path::new("/var/run/docker.sock"), Last::File).unwrap();
        assert_eq!(rel(&root, &f), Path::new("run/docker.sock"));
        let f = open(r.as_fd(), Path::new("/var/sock"), Last::File).unwrap();
        assert_eq!(rel(&root, &f), Path::new("run/app.sock"));
        // An existing file is used as it is.
        let again = open(r.as_fd(), Path::new("/var/sock"), Last::File).unwrap();
        assert!(!again.created);
    }

    #[test]
    fn symlink_loop_is_an_error() {
        let (_t, root, _o) = setup("loop");
        symlink("/b", root.join("a")).unwrap();
        symlink("/a", root.join("b")).unwrap();
        let r = open_root(&root).unwrap();
        let e = open(r.as_fd(), Path::new("/a/x"), Last::Dir).err().unwrap();
        assert_eq!(e.raw_os_error(), None); // wrapped with the path
        assert!(e.to_string().contains(&format!("os error {}", libc::ELOOP)), "{e}");
    }

    #[test]
    fn file_in_the_way_is_not_a_directory() {
        let (_t, root, _o) = setup("notdir");
        fs::write(root.join("f"), "").unwrap();
        let r = open_root(&root).unwrap();
        assert!(open(r.as_fd(), Path::new("/f/x"), Last::Dir).is_err());
        assert!(open(r.as_fd(), Path::new("/missing"), Last::Exists).is_err());
    }

    #[test]
    fn components_drop_root_and_dots() {
        assert_eq!(components_of(Path::new("/a/./b/../c/")), ["a", "b", "..", "c"].map(OsString::from));
    }
}
