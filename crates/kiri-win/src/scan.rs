//! An image's page, Layers and Security, as crates/kiri/src/builds.rs
//! (`image_layers`) and scan.rs are on the Mac. Kiri's store is inside WSL,
//! so the engine there reads the layers and runs Grype, keeping the scans
//! beside the store (`/kiri/images/…`, crates/kiri/src/daemon/api.rs); this
//! asks it, saves an SBOM to Downloads and opens advisory pages.

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;

use crate::client::{self, Response};
use crate::engine::path_escape;

/// Advisory pages from the scans shown so far: the only addresses `open_advisory` opens.
static LINKS: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// What a 404 means here: the engine in WSL predates these requests.
fn failed(r: &Response) -> io::Error {
    io::Error::other(if r.status == 404 { "Kiri's engine in WSL is older than this window: update Kiri".to_owned() } else { r.error() })
}

/// `/kiri/images/<what>?name=…[&at=…]`, as the window's own request.
fn get(what: &str, name: &str, at: Option<u64>) -> io::Result<Response> {
    let at = at.map(|a| format!("&at={a}")).unwrap_or_default();
    let r = client::request_with("GET", &format!("/kiri/images/{what}?name={}{at}", path_escape(name)), None, true)?;
    if !r.ok() {
        return Err(failed(&r));
    }
    Ok(r)
}

/// Keeps the advisory pages a scan lists, for `open_advisory`.
fn remember(scan: &Value) {
    let mut links = LINKS.lock().unwrap_or_else(|e| e.into_inner());
    let links = links.get_or_insert_with(HashSet::new);
    for v in scan["vulns"].as_array().into_iter().flatten() {
        if let Some(u) = v["url"].as_str().filter(|u| u.starts_with("https://")) {
            links.insert(u.to_owned());
        }
    }
}

/// The image's layers and config: {name, images, materials}.
pub fn image_layers(name: &str) -> io::Result<Value> {
    Ok(get("layers", name, None)?.json())
}

/// Scans the image; `progress(phase, percent)` as the Mac's scan::scan.
pub fn scan(name: &str, progress: &mut dyn FnMut(&str, u8)) -> io::Result<Value> {
    let mut end = None;
    let status = client::post_lines(&format!("/kiri/images/scan?name={}", path_escape(name)), |v| {
        if let Some(phase) = v["phase"].as_str() {
            progress(phase, v["pct"].as_u64().unwrap_or(0).min(100) as u8);
        } else if let Some(e) = v["error"].as_str() {
            end = Some(Err(io::Error::other(e.to_owned())));
        } else if let Some(m) = v["message"].as_str() {
            end = Some(Err(io::Error::other(m.to_owned())));
        } else if v.get("result").is_some() {
            end = Some(Ok(v["result"].clone()));
        }
    })?;
    if status == 404 {
        return Err(failed(&Response { status, body: Vec::new() }));
    }
    let res = end.unwrap_or_else(|| Err(io::Error::other("the scan stopped before it finished")))?;
    remember(&res);
    Ok(res)
}

/// The image's scans for the history list: {id, scans}.
pub fn history(name: &str) -> io::Result<Value> {
    Ok(get("scans", name, None)?.json())
}

/// One of the image's scans in full (the newest when `at` is None), or None.
pub fn open(name: &str, at: Option<u64>) -> io::Result<Option<Value>> {
    let v = get("scan", name, at)?.json();
    if v.is_null() {
        return Ok(None);
    }
    remember(&v);
    Ok(Some(v))
}

/// Saves one scan's SBOM to Downloads as `<file>.cdx.json`, and returns where.
pub fn save_sbom(name: &str, at: u64, file: &str) -> io::Result<PathBuf> {
    let bytes = get("sbom", name, Some(at))?.body;
    let stem: String = file.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '-' }).collect();
    let stem = if stem.trim_matches(['-', '.']).is_empty() { "image".to_owned() } else { stem };
    let downloads = std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Downloads")).ok_or_else(|| io::Error::other("no Downloads folder"))?;
    // create_new takes the first free name, with no gap between a check and the write.
    let mut n = 1;
    loop {
        // Not CON.cdx.json and the like, which Windows reserves.
        let to = downloads.join(crate::detail::safe_filename(&if n == 1 { format!("{stem}.cdx.json") } else { format!("{stem} {n}.cdx.json") }));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&to) {
            Ok(mut f) => {
                io::Write::write_all(&mut f, &bytes)?;
                return Ok(to);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && n < 1000 => n += 1,
            Err(e) => return Err(e),
        }
    }
}

/// Opens an advisory page, only if a scan listed it.
pub fn open_advisory(url: &str) {
    if LINKS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|l| l.contains(url)) {
        crate::app::open_url(url);
    }
}
