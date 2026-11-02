//! Telling official Kiri builds from forks and scripts.
//!
//! Each release is built with a random 32-byte build key compiled in. The
//! edge knows the keys by id (`id:hex`) and challenges clients that set
//! [`crate::FEATURE_ATTEST`] with a fresh random nonce; the client answers
//! with [`proof`]: HMAC-SHA-256 under the build key over the nonce, the
//! SHA-256 of its account token and its device id. The nonce makes every
//! answer single-use, and the token and device bind it to this session, so
//! an answer can't be replayed or relayed for someone else's.
//!
//! A key in a binary can be extracted, so this raises the bar rather than
//! proving anything; a leaked key is revoked by removing its id on the edge,
//! and other releases keep working.

use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

/// Bytes in a build key.
pub const KEY_LEN: usize = 32;
/// Bytes in a `Challenge` nonce.
pub const NONCE_LEN: usize = 32;
/// Bytes in an `Attest` MAC.
pub const MAC_LEN: usize = 32;
/// Longest key id either side accepts.
pub const MAX_KEY_ID: usize = 32;

/// Separates these MACs from any other use of the key.
const CONTEXT: &[u8] = b"kiri-share attest v1";

fn mac(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], token: &str, device: &str) -> Hmac<Sha256> {
    let mut m = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC takes any key length");
    m.update(CONTEXT);
    m.update(nonce);
    m.update(&Sha256::digest(token.as_bytes()));
    m.update(&(device.len() as u32).to_le_bytes());
    m.update(device.as_bytes());
    m
}

/// The client's answer to `nonce` for the session it opened with `token`
/// and `device` (exactly as sent in its `Hello`).
pub fn proof(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], token: &str, device: &str) -> [u8; MAC_LEN] {
    mac(key, nonce, token, device).finalize().into_bytes().into()
}

/// Whether `answer` is [`proof`] under `key`, compared in constant time.
pub fn verify(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], token: &str, device: &str, answer: &[u8; MAC_LEN]) -> bool {
    mac(key, nonce, token, device).verify_slice(answer).is_ok()
}

/// Parses `id:hex`, the form keys take in configuration: an id of 1 to
/// [`MAX_KEY_ID`] letters, digits, `-`, `_` or `.`, and 64 hex digits.
pub fn parse_key(s: &str) -> Option<(&str, [u8; KEY_LEN])> {
    let (id, hex) = s.trim().split_once(':')?;
    let id_ok = !id.is_empty() && id.len() <= MAX_KEY_ID && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b));
    if !id_ok || hex.len() != KEY_LEN * 2 {
        return None;
    }
    let mut key = [0u8; KEY_LEN];
    for (i, k) in key.iter_mut().enumerate() {
        *k = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some((id, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; KEY_LEN] = [0x11; KEY_LEN];
    const NONCE: [u8; NONCE_LEN] = [0x22; NONCE_LEN];

    #[test]
    fn proof_verifies() {
        let p = proof(&KEY, &NONCE, "ks_token", "d_device");
        assert!(verify(&KEY, &NONCE, "ks_token", "d_device", &p));
    }

    #[test]
    fn proof_is_bound_to_key_nonce_token_and_device() {
        let p = proof(&KEY, &NONCE, "ks_token", "d_device");
        assert!(!verify(&[0x12; KEY_LEN], &NONCE, "ks_token", "d_device", &p), "other key");
        assert!(!verify(&KEY, &[0x23; NONCE_LEN], "ks_token", "d_device", &p), "other nonce (replay)");
        assert!(!verify(&KEY, &NONCE, "ks_other", "d_device", &p), "other token");
        assert!(!verify(&KEY, &NONCE, "ks_token", "d_other", &p), "other device");
        let mut bad = p;
        bad[0] ^= 1;
        assert!(!verify(&KEY, &NONCE, "ks_token", "d_device", &bad), "flipped bit");
    }

    #[test]
    fn device_length_is_framed() {
        // Without the length, moving bytes between fields could collide.
        assert_ne!(proof(&KEY, &NONCE, "t", "ab"), proof(&KEY, &NONCE, "t", "a"));
    }

    #[test]
    fn known_answer() {
        // Pinned so the edge and every client build agree on the construction.
        let p = proof(&KEY, &NONCE, "ks_token", "d_device");
        let mut m = <Hmac<Sha256> as KeyInit>::new_from_slice(&KEY).unwrap();
        let mut msg = alloc::vec::Vec::new();
        msg.extend_from_slice(b"kiri-share attest v1");
        msg.extend_from_slice(&NONCE);
        msg.extend_from_slice(&Sha256::digest(b"ks_token"));
        msg.extend_from_slice(&8u32.to_le_bytes());
        msg.extend_from_slice(b"d_device");
        m.update(&msg);
        assert_eq!(p, <[u8; MAC_LEN]>::from(m.finalize().into_bytes()));
    }

    #[test]
    fn parses_keys() {
        let hex = "00112233445566778899aabbccddeeff00112233445566778899AABBCCDDEEFF";
        let s = alloc::format!("r1:{hex}");
        let (id, key) = parse_key(&s).unwrap();
        assert_eq!(id, "r1");
        assert_eq!((key[0], key[1], key[15], key[31]), (0x00, 0x11, 0xff, 0xff));
        assert!(parse_key(&alloc::format!(" 2026.10-a_b:{hex}\n")).is_some());
        assert!(parse_key(hex).is_none(), "no id");
        assert!(parse_key(&alloc::format!(":{hex}")).is_none(), "empty id");
        assert!(parse_key(&alloc::format!("a b:{hex}")).is_none(), "bad id");
        assert!(parse_key(&alloc::format!("{}:{hex}", "x".repeat(33))).is_none(), "long id");
        assert!(parse_key("r1:0011").is_none(), "short key");
        assert!(parse_key(&alloc::format!("r1:{}", "zz".repeat(32))).is_none(), "not hex");
        assert!(parse_key(&alloc::format!("r1:{}", "é".repeat(32))).is_none(), "not ASCII");
    }
}
