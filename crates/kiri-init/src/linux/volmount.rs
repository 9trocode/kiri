//! Volumes with the local driver's options (`type=nfs`, `type=tmpfs`,
//! `type=none,o=bind`, `type=cifs`): the guest mounts the device at the
//! volume's own path the first time a container starts with it, as Docker's
//! local driver does, and the container binds that path in like any other
//! volume. The mount stays until the volume is removed or the VM stops.

use std::ffi::CString;
use std::fs;
use std::io;

/// Where volumes live; nothing outside it is ever mounted over.
const ROOT: &str = "/var/lib/kiri/volumes/";

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, what.to_owned())
}

/// Whether `path` is a mount point in this mount namespace.
pub fn mounted(path: &str) -> bool {
    let Ok(info) = fs::read_to_string("/proc/self/mountinfo") else { return false };
    info.lines().any(|l| l.split(' ').nth(4) == Some(path))
}

/// Mounts the volume at `path` unless something already is: `kind` is the
/// filesystem (`none` for a bind), `flags_hex` and `data` are `o` split as
/// mount(2) takes it. Errors read as Docker's.
pub fn ensure(path: &str, kind: &str, device: &str, flags_hex: &str, data: &str) -> io::Result<()> {
    if !path.starts_with(ROOT) || path.split('/').any(|c| c == "..") {
        return Err(invalid("volume mount outside the volume store"));
    }
    if mounted(path) {
        return Ok(());
    }
    let flags = u64::from_str_radix(flags_hex, 16).map_err(|_| invalid("bad volume mount flags"))? as libc::c_ulong;
    let fail = |msg: String| io::Error::other(format!("error while mounting volume '{path}': {msg}"));
    if kind.starts_with("nfs") {
        // The daemon resolved the server; one it could not is reported here,
        // at start, as Docker does.
        if let Some(addr) = data.split(',').find_map(|w| w.strip_prefix("addr=")) {
            if addr.parse::<std::net::IpAddr>().is_err() {
                return Err(fail(format!("error resolving passed in network volume address: lookup {addr}: no such host")));
            }
        }
    }
    let bind = flags & libc::MS_BIND != 0;
    let c = |s: &str| CString::new(s).map_err(|_| invalid("volume option contains NUL"));
    let (src, target, fstype) = (c(device)?, c(path)?, c(if bind { "none" } else { kind })?);
    let data_c = c(data)?;
    let mount = |flags: libc::c_ulong, data: Option<&CString>| {
        // SAFETY: valid NUL-terminated strings, or null for no data.
        let r = unsafe { libc::mount(src.as_ptr(), target.as_ptr(), fstype.as_ptr(), flags, data.map_or(std::ptr::null(), |d| d.as_ptr().cast())) };
        if r == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
    };
    let result = if bind {
        // A bind takes no other flags at first; read-only is a remount.
        mount(flags & (libc::MS_BIND | libc::MS_REC), None).and_then(|()| {
            if flags & libc::MS_RDONLY != 0 { mount(libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY | (flags & libc::MS_REC), None) } else { Ok(()) }
        })
    } else {
        mount(flags, (!data.is_empty()).then_some(&data_c))
    };
    result.map_err(|e| {
        // As Docker's mount error: `mount src:target, flags: 0x…, data: …: errno`,
        // with any password hidden.
        let mut msg = format!("failed to mount local volume: mount {device}:{path}");
        if flags != 0 {
            msg.push_str(&format!(", flags: 0x{flags:x}"));
        }
        if !data.is_empty() {
            msg.push_str(&format!(", data: {}", hide_password(data)));
        }
        msg.push_str(&format!(": {}", super::hostopts::errno_text(&e)));
        io::Error::new(e.kind(), format!("error while mounting volume '{path}': {msg}"))
    })
}

fn hide_password(data: &str) -> String {
    data.split(',').map(|w| if w.starts_with("password=") { "password=********" } else { w }).collect::<Vec<_>>().join(",")
}

/// Unmounts whatever is mounted at a volume's path before its data is
/// deleted, so removing a volume never deletes the NFS export or the host
/// folder behind it. False if something is still mounted there.
pub fn release(path: &str) -> bool {
    for _ in 0..16 {
        if !mounted(path) {
            return true;
        }
        if super::sys::umount_lazy(std::path::Path::new(path)).is_err() {
            break;
        }
    }
    !mounted(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_stay_out_of_errors() {
        assert_eq!(hide_password("username=u,password=secret,vers=3.0"), "username=u,password=********,vers=3.0");
    }

    #[test]
    fn only_inside_the_volume_store() {
        assert!(ensure("/etc", "tmpfs", "tmpfs", "0", "").is_err());
        assert!(ensure("/var/lib/kiri/volumes/../../etc/_data", "tmpfs", "tmpfs", "0", "").is_err());
    }
}
