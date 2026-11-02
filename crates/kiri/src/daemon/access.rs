//! Who may use the engine's socket on a Linux host, without signing in again.
//!
//! The socket is 0660 root:kiri, so members of the `kiri` group may use it, as
//! the docker group may use dockerd's. But a group someone was just added to
//! only applies to sessions that start after: until they log out and back in,
//! the window and every terminal they have open are refused. So each member
//! also gets their own entry in the socket's access list (a POSIX ACL), which
//! applies at once, to every session they have.
//!
//! The daemon writes the list when it makes the socket, and `kiri host-access`
//! (run as root, from the window's onboarding through pkexec) writes it again
//! after adding someone. Someone taken out of the group keeps their entry
//! until the engine restarts and the socket is made again.

use std::ffi::{CStr, CString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// The users in `group` by name (its member list, not primary-group users).
pub fn members(group: &str) -> io::Result<Vec<String>> {
    let name = CString::new(group).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: getgrnam with a valid C string; its result is copied out at once,
    // before anything else could reuse its static storage.
    unsafe {
        let g = libc::getgrnam(name.as_ptr());
        if g.is_null() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("no group {group}")));
        }
        let mut out = Vec::new();
        let mut m = (*g).gr_mem;
        while !m.is_null() && !(*m).is_null() {
            out.push(CStr::from_ptr(*m).to_string_lossy().into_owned());
            m = m.add(1);
        }
        Ok(out)
    }
}

/// A user's numeric id, if the account exists.
pub fn uid(user: &str) -> Option<u32> {
    let name = CString::new(user).ok()?;
    // SAFETY: getpwnam with a valid C string; the uid is read at once.
    unsafe {
        let p = libc::getpwnam(name.as_ptr());
        (!p.is_null()).then(|| (*p).pw_uid)
    }
}

/// Gives every member of `group` read-write access to `socket` by name, so a
/// member just added can use it without signing in again. Returns how many.
pub fn grant(socket: &Path, group: &str) -> io::Result<usize> {
    let uids: Vec<u32> = members(group)?.iter().filter_map(|u| uid(u)).collect();
    set(socket, &uids)?;
    Ok(uids.len())
}

/// Writes the socket's access list: its owner and group (rw, as the mode
/// says), each of `uids` (rw), nobody else.
pub fn set(socket: &Path, uids: &[u32]) -> io::Result<()> {
    let blob = acl(uids);
    let path = CString::new(socket.as_os_str().as_bytes()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: both strings are valid and NUL-terminated; blob outlives the call.
    let r = unsafe { libc::setxattr(path.as_ptr(), c"system.posix_acl_access".as_ptr(), blob.as_ptr().cast(), blob.len(), 0) };
    if r == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

/// Whether a failure to write the list means this system can't keep one
/// (a filesystem without ACLs): then only signing in again lets a new member in.
pub fn unsupported(e: &io::Error) -> bool {
    e.raw_os_error() == Some(libc::EOPNOTSUPP)
}

/// The kernel's form of an access list (`system.posix_acl_access`): a version,
/// then entries of tag, permissions and id, in tag order and, within a tag,
/// in id order.
fn acl(uids: &[u32]) -> Vec<u8> {
    const USER_OBJ: u16 = 0x01;
    const USER: u16 = 0x02;
    const GROUP_OBJ: u16 = 0x04;
    const MASK: u16 = 0x10;
    const OTHER: u16 = 0x20;
    const NO_ID: u32 = u32::MAX;
    const RW: u16 = 6;
    fn entry(out: &mut Vec<u8>, tag: u16, perm: u16, id: u32) {
        out.extend(tag.to_le_bytes());
        out.extend(perm.to_le_bytes());
        out.extend(id.to_le_bytes());
    }
    let mut ids: Vec<u32> = uids.iter().copied().filter(|&u| u != 0).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut out = 2u32.to_le_bytes().to_vec();
    entry(&mut out, USER_OBJ, RW, NO_ID);
    for id in ids {
        entry(&mut out, USER, RW, id);
    }
    entry(&mut out, GROUP_OBJ, RW, NO_ID);
    entry(&mut out, MASK, RW, NO_ID);
    entry(&mut out, OTHER, 0, NO_ID);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acl_lists_each_user_once_in_order_and_not_root() {
        let b = acl(&[1001, 0, 1000, 1001]);
        assert_eq!(&b[..4], &[2, 0, 0, 0]);
        let entries: Vec<(u16, u16, u32)> = b[4..]
            .chunks(8)
            .map(|c| (u16::from_le_bytes([c[0], c[1]]), u16::from_le_bytes([c[2], c[3]]), u32::from_le_bytes([c[4], c[5], c[6], c[7]])))
            .collect();
        assert_eq!(entries, vec![(1, 6, u32::MAX), (2, 6, 1000), (2, 6, 1001), (4, 6, u32::MAX), (0x10, 6, u32::MAX), (0x20, 0, u32::MAX)]);
    }

    /// The kernel takes the list and lets a user who isn't in the socket's
    /// group in by it, at once. Needs root (cargo test -- --ignored, as root).
    #[test]
    #[ignore]
    fn a_listed_user_gets_in_and_others_dont() {
        use std::os::unix::fs::PermissionsExt;
        let dir = Path::new("/dev/shm").join(format!("kiri-acl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let sock = dir.join("s.sock");
        let _l = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        std::fs::set_permissions(&sock, std::fs::Permissions::from_mode(0o660)).unwrap();
        // As uid `as_uid` (no groups): can it connect?
        let try_as = |as_uid: u32| {
            // SAFETY: fork in a test; the child only drops ids, connects and exits.
            unsafe {
                let pid = libc::fork();
                if pid == 0 {
                    libc::setgroups(0, std::ptr::null());
                    libc::setgid(as_uid);
                    libc::setuid(as_uid);
                    libc::_exit(if std::os::unix::net::UnixStream::connect(&sock).is_ok() { 0 } else { 1 });
                }
                let mut status = 0;
                libc::waitpid(pid, &mut status, 0);
                libc::WEXITSTATUS(status) == 0
            }
        };
        assert!(!try_as(65534), "nobody isn't let in before the list");
        set(&sock, &[65534]).unwrap();
        assert!(try_as(65534), "a listed user gets in");
        assert!(!try_as(65533), "an unlisted one doesn't");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn acl_without_users_is_the_plain_mode() {
        assert_eq!(acl(&[]).len(), 4 + 4 * 8);
    }
}
