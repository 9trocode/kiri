// Compiles the Kiri Share build key into official releases: `include!`d
// by the build scripts of `kiri` and `kiri-win`, which both carry
// `share.rs`.
// 
// Release builds set `KIRI_SHARE_BUILD_KEY=id:hex` (64 hex digits); the
// edge uses it to tell official builds from forks (see
// `kiri-share-proto::attest`). Without it Kiri builds and works the same,
// and its tunnels show as unverified. The key is stored masked, so it isn't
// a plain string in the binary; that only slows down someone looking.

/// Writes `$OUT_DIR/share_build_key.rs`, which `share.rs` includes.
fn write_share_build_key() {
    use std::fmt::Write as _;
    use std::hash::{DefaultHasher, Hash, Hasher};

    println!("cargo:rerun-if-env-changed=KIRI_SHARE_BUILD_KEY");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("share_build_key.rs");
    let code = match std::env::var("KIRI_SHARE_BUILD_KEY").ok().filter(|v| !v.trim().is_empty()) {
        None => "const BUILD_KEY: Option<(&str, [u8; 32], [u8; 32])> = None;\n".to_owned(),
        Some(v) => {
            let (id, key) = parse_share_key(&v).unwrap_or_else(|| panic!("KIRI_SHARE_BUILD_KEY must be id:hex with 64 hex digits"));
            // A pad derived from the id, so builds stay reproducible.
            let mut pad = [0u8; 32];
            for (i, chunk) in pad.chunks_mut(8).enumerate() {
                let mut h = DefaultHasher::new();
                (id, i, "kiri share pad").hash(&mut h);
                chunk.copy_from_slice(&h.finish().to_le_bytes());
            }
            let masked: Vec<u8> = key.iter().zip(pad).map(|(k, p)| k ^ p).collect();
            let mut s = String::new();
            writeln!(s, "const BUILD_KEY: Option<(&str, [u8; 32], [u8; 32])> = Some(({id:?}, {masked:?}, {pad:?}));").unwrap();
            s
        }
    };
    std::fs::write(out, code).expect("write share_build_key.rs");
}

fn parse_share_key(s: &str) -> Option<(&str, [u8; 32])> {
    let (id, hex) = s.trim().split_once(':')?;
    if id.is_empty() || id.len() > 32 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)) || hex.len() != 64 {
        return None;
    }
    let mut key = [0u8; 32];
    for (i, k) in key.iter_mut().enumerate() {
        *k = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some((id, key))
}
