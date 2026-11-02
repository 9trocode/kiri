//! The guest's layer store: compressed layer blobs arrive from the host over
//! vsock and are unpacked once into `layers/<hex>/`, ready to be stacked by
//! overlayfs. A layer directory only appears, by rename, once it is complete.

use std::fs;
use std::io::{self, BufReader, Read};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

use kiri_proto::{LayerBegin, LayerStored};

use super::sys;

pub fn hex(digest: &str) -> io::Result<&str> {
    let h = digest.strip_prefix("sha256:").unwrap_or(digest);
    if h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("bad layer digest {digest}")));
    }
    Ok(h)
}

pub fn dir(root: &Path, digest: &str) -> io::Result<PathBuf> {
    Ok(root.join(hex(digest)?))
}

pub fn present(root: &Path, digest: &str) -> bool {
    dir(root, digest).is_ok_and(|d| d.is_dir())
}

/// Reads exactly `begin.size` bytes of blob from `stream` and unpacks them.
/// The stream stays in sync even if unpacking fails part-way.
pub fn receive(stream: &mut impl Read, begin: &LayerBegin<'_>, root: &Path) -> io::Result<LayerStored> {
    let started = Instant::now();
    let final_dir = dir(root, begin.digest)?;
    let tmp = root.join(format!(".tmp-{}", hex(begin.digest)?));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp)?;

    let mut limited = stream.take(begin.size);
    let result = unpack(&mut limited, begin.media_type, &tmp, begin.size);
    // Whatever happened, consume the rest of the blob so the next frame lines up.
    io::copy(&mut limited, &mut io::sink())?;

    match result {
        Ok(unpacked_bytes) => {
            if final_dir.exists() {
                let _ = fs::remove_dir_all(&tmp);
            } else {
                fs::rename(&tmp, &final_dir)?;
            }
            Ok(LayerStored { unpacked_bytes, elapsed_ns: started.elapsed().as_nanos() as u64 })
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp);
            Err(e)
        }
    }
}

/// The most a layer may expand to. The blob's digest covers only its
/// compressed bytes, so a tiny, digest-valid layer could otherwise expand
/// without limit onto the disk (on a Linux host or in WSL, the host's own).
/// 256x the compressed size, never below 1 GiB nor above 64 GiB.
fn expansion_cap(compressed: u64) -> u64 {
    compressed.saturating_mul(256).clamp(1 << 30, 64 << 30)
}

/// A reader that fails once more than `left` bytes have come through.
struct Capped<R> {
    inner: R,
    left: u64,
}

impl<R: Read> Read for Capped<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n as u64 > self.left {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "layer expands beyond its allowed size"));
        }
        self.left -= n as u64;
        Ok(n)
    }
}

fn unpack(blob: &mut impl Read, media_type: &str, dest: &Path, compressed: u64) -> io::Result<u64> {
    let buffered = BufReader::with_capacity(256 * 1024, blob);
    let left = expansion_cap(compressed);
    if media_type.ends_with("gzip") {
        untar(Capped { inner: flate2::read::MultiGzDecoder::new(buffered), left }, dest)
    } else if media_type.ends_with("zstd") {
        let decoder = ruzstd::decoding::StreamingDecoder::new(buffered)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("zstd: {e}")))?;
        untar(Capped { inner: decoder, left }, dest)
    } else {
        untar(Capped { inner: buffered, left }, dest)
    }
}

/// Unpacks a layer tarball, translating OCI whiteouts into the form overlayfs
/// reads: `.wh.name` becomes a 0:0 character device `name`, and
/// `.wh..wh..opq` marks its directory opaque.
fn untar(reader: impl Read, dest: &Path) -> io::Result<u64> {
    let mut archive = tar::Archive::new(reader);
    archive.set_preserve_permissions(true);
    archive.set_preserve_ownerships(true);
    archive.set_preserve_mtime(true);
    archive.set_unpack_xattrs(true);
    archive.set_overwrite(true);
    let mut total = 0u64;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let name = path.file_name().map(|n| n.as_bytes()).unwrap_or_default();
        if let Some(hidden) = name.strip_prefix(b".wh.") {
            // `.wh.`, `.wh..` and `.wh...` would hide the directory itself or its parent.
            if matches!(hidden, b"" | b"." | b"..") {
                return Err(escapes());
            }
            let parent = whiteout_dir(dest, path.parent().unwrap_or(Path::new("")))?;
            if hidden == b".wh..opq" {
                let p = sys::cstr(&parent)?;
                // SAFETY: valid NUL-terminated path, name and a 1-byte value.
                let ret = unsafe {
                    libc::setxattr(p.as_ptr(), c"trusted.overlay.opaque".as_ptr(), b"y".as_ptr().cast(), 1, 0)
                };
                sys::check(ret as _, "mark opaque")?;
            } else {
                let target = parent.join(std::ffi::OsStr::from_bytes(hidden));
                let _ = fs::remove_file(&target);
                let _ = fs::remove_dir_all(&target);
                sys::mknod(&sys::cstr(&target)?, libc::S_IFCHR, 0, 0)?;
            }
            continue;
        }
        total += entry.size();
        entry.unpack_in(dest)?;
    }
    Ok(total)
}

fn escapes() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "whiteout escapes the layer")
}

/// The directory under `dest` a whiteout lives in, made if missing. Every
/// step must be a real directory of this layer: an earlier entry could have
/// made one a symlink, which a check of the path's text cannot see and which
/// would carry the whiteout outside the layer.
fn whiteout_dir(dest: &Path, parent: &Path) -> io::Result<PathBuf> {
    let mut dir = dest.to_path_buf();
    for part in parent.components() {
        match part {
            std::path::Component::Normal(name) => dir.push(name),
            std::path::Component::CurDir => continue,
            _ => return Err(escapes()),
        }
        match fs::symlink_metadata(&dir) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(escapes()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&dir)?,
            Err(e) => return Err(e),
        }
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete header: tar-rs refuses to unpack one with empty uid/gid.
    fn header(size: u64, mode: u32) -> tar::Header {
        let mut h = tar::Header::new_gnu();
        h.set_size(size);
        h.set_mode(mode);
        h.set_uid(0);
        h.set_gid(0);
        h.set_mtime(0);
        h.set_cksum();
        h
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("kiri-layers-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A layer whose first entry makes `foo` a symlink to `/` and whose
    /// second writes `foo/x` must not write outside the layer directory.
    #[test]
    fn symlink_parent_cannot_escape_layer() {
        let mut b = tar::Builder::new(Vec::new());
        let mut h = header(0, 0o777);
        h.set_entry_type(tar::EntryType::Symlink);
        b.append_link(&mut h, "foo", "/").unwrap();
        let mut h = header(1, 0o644);
        b.append_data(&mut h, "foo/x", &b"x"[..]).unwrap();
        let bytes = b.into_inner().unwrap();
        let dest = scratch("escape");
        let err = untar(Capped { inner: &bytes[..], left: u64::MAX }, &dest).expect_err("writing through a symlinked parent must fail");
        // The refusal must be the path check, not some other unpack error.
        assert!(err.to_string().contains("outside"), "unexpected error: {err}");
        assert!(!Path::new("/x").is_file() || fs::read_to_string("/x").map_or(true, |c| c != "x"), "escaped to /x");
        let _ = fs::remove_dir_all(&dest);
    }

    fn whiteout_layer(entries: &[(&str, Option<&str>)]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        for (name, link) in entries {
            if let Some(to) = link {
                let mut h = header(0, 0o777);
                h.set_entry_type(tar::EntryType::Symlink);
                b.append_link(&mut h, name, to).unwrap();
            } else {
                let mut h = header(0, 0o644);
                b.append_data(&mut h, name, &[][..]).unwrap();
            }
        }
        b.into_inner().unwrap()
    }

    /// A whiteout under a symlink to a directory outside the layer must not
    /// delete, replace or mark anything there.
    #[test]
    fn whiteout_through_symlink_stays_in_layer() {
        let outside = scratch("wh-outside");
        fs::write(outside.join("victim"), "keep").unwrap();
        let to = outside.to_str().unwrap();
        for hidden in ["foo/.wh.victim", "foo/.wh..wh..opq"] {
            let dest = scratch("wh-escape");
            let bytes = whiteout_layer(&[("foo", Some(to)), (hidden, None)]);
            let err = untar(Capped { inner: &bytes[..], left: u64::MAX }, &dest).expect_err(hidden);
            assert!(err.to_string().contains("escapes"), "{hidden}: unexpected error: {err}");
            assert_eq!(fs::read_to_string(outside.join("victim")).unwrap(), "keep", "{hidden}");
            assert!(!outside.join(&hidden[4..]).exists(), "{hidden}: placeholder written outside");
            let _ = fs::remove_dir_all(&dest);
        }
        let _ = fs::remove_dir_all(&outside);
    }

    /// Whiteouts naming the directory itself or its parent are refused
    /// before anything is removed.
    #[test]
    fn whiteout_of_dot_or_dotdot_is_refused() {
        for hidden in ["d/.wh.", "d/.wh..", "d/.wh..."] {
            let dest = scratch("wh-dots");
            fs::create_dir_all(dest.join("d")).unwrap();
            fs::write(dest.join("d/keep"), "x").unwrap();
            let bytes = whiteout_layer(&[(hidden, None)]);
            let err = untar(Capped { inner: &bytes[..], left: u64::MAX }, &dest).expect_err(hidden);
            assert!(err.to_string().contains("escapes"), "{hidden}: unexpected error: {err}");
            assert!(dest.join("d/keep").is_file(), "{hidden}");
            let _ = fs::remove_dir_all(&dest);
        }
    }

    /// A whiteout whose own path is absolute (the tar builder won't write
    /// one, so the header is patched by hand) must not reach outside.
    #[test]
    fn whiteout_path_cannot_escape_layer() {
        let base = scratch("wh-abs");
        let dest = base.join("layer");
        fs::create_dir(&dest).unwrap();
        fs::write(base.join("keep"), "x").unwrap();
        let path = format!("{}/.wh.keep", base.display());
        let mut b = tar::Builder::new(Vec::new());
        b.append_data(&mut header(0, 0o644), "a".repeat(path.len()), &[][..]).unwrap();
        let mut bytes = b.into_inner().unwrap();
        bytes[..100].fill(0);
        bytes[..path.len()].copy_from_slice(path.as_bytes());
        bytes[148..156].fill(b' ');
        let checksum: u32 = bytes[..512].iter().map(|b| u32::from(*b)).sum();
        bytes[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
        assert!(untar(Capped { inner: &bytes[..], left: u64::MAX }, &dest).is_err());
        assert!(base.join("keep").is_file());
        let _ = fs::remove_dir_all(&base);
    }

    /// An ordinary whiteout becomes overlayfs's 0:0 character device and
    /// leaves no `.wh.` file behind. mknod needs root, as in the guest.
    #[test]
    fn whiteout_becomes_char_device() {
        // SAFETY: a plain getter.
        if unsafe { libc::geteuid() } != 0 {
            eprintln!("skipped: needs root for mknod");
            return;
        }
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let dest = scratch("wh-ok");
        fs::create_dir_all(dest.join("etc")).unwrap();
        fs::write(dest.join("etc/gone"), "x").unwrap();
        let bytes = whiteout_layer(&[("etc/.wh.gone", None)]);
        untar(Capped { inner: &bytes[..], left: u64::MAX }, &dest).unwrap();
        let m = fs::symlink_metadata(dest.join("etc/gone")).unwrap();
        assert!(m.file_type().is_char_device() && m.rdev() == 0, "not a 0:0 char device");
        assert!(!dest.join("etc/.wh.gone").exists(), "placeholder left behind");
        let _ = fs::remove_dir_all(&dest);
        // As `docker commit` writes them: no owner or time, which tar-rs
        // would refuse to unpack. A whiteout is never unpacked, so it works,
        // and missing parents are made.
        let mut b = tar::Builder::new(Vec::new());
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Regular);
        h.set_size(0);
        h.set_mode(0o644);
        b.append_data(&mut h, "usr/share/.wh.doc", std::io::empty()).unwrap();
        let bytes = b.into_inner().unwrap();
        let dest = scratch("wh-bare");
        untar(Capped { inner: &bytes[..], left: u64::MAX }, &dest).unwrap();
        assert!(fs::symlink_metadata(dest.join("usr/share/doc")).unwrap().file_type().is_char_device());
        let _ = fs::remove_dir_all(&dest);
    }

    /// A layer that expands past its cap is refused.
    #[test]
    fn expansion_is_capped() {
        let mut b = tar::Builder::new(Vec::new());
        let data = vec![0u8; 4096];
        let mut h = header(data.len() as u64, 0o644);
        b.append_data(&mut h, "big", &data[..]).unwrap();
        let bytes = b.into_inner().unwrap();
        // As in receive(): a layer that fails is discarded, never reused.
        let capped = scratch("cap");
        assert!(untar(Capped { inner: &bytes[..], left: 1024 }, &capped).is_err());
        let _ = fs::remove_dir_all(&capped);
        let fresh = scratch("cap-ok");
        assert_eq!(untar(Capped { inner: &bytes[..], left: u64::MAX }, &fresh).unwrap(), 4096);
        let _ = fs::remove_dir_all(&fresh);
        assert_eq!(expansion_cap(1), 1 << 30);
        assert_eq!(expansion_cap(1 << 40), 64 << 30);
    }
}
