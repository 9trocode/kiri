//! Kiri's icon and version in kiri-win.exe. The icon is for Explorer, Start
//! and a pinned taskbar button (the running window sets its own). The
//! version is for Explorer's Details and for Windows Installer, which
//! replaces a file by comparing versions: a file with none it judges by
//! its dates, and keeps one that looks hand-written, so an upgrade could
//! leave an old kiri-win.exe behind. Windows builds only.

use std::path::Path;

// The Kiri Share build key, as crates/kiri/build.rs compiles it: kiri-win
// carries share.rs too, for the window's Share.
include!("../kiri/build_share_key.rs");

fn main() {
    println!("cargo:rerun-if-changed=../kiri/build_share_key.rs");
    write_share_build_key();
    // The bundle files' SHA-256s, set by `cargo xtask windows`.
    println!("cargo:rerun-if-env-changed=KIRI_BUNDLE_SHA256");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-changed=../../brand/icon/windows/kiri.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../brand/icon/windows/kiri.ico").canonicalize().expect("brand/icon/windows/kiri.ico");
    // The resource compiler takes forward slashes and no \\?\ prefix.
    let ico = ico.to_string_lossy().trim_start_matches(r"\\?\").replace('\\', "/");
    let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let commas = {
        let mut parts: Vec<String> = version.split(['.', '-', '+']).take(3).map(|p| p.chars().take_while(char::is_ascii_digit).collect()).collect();
        parts.resize(4, "0".to_owned());
        parts.join(",")
    };
    let rc = format!(
        r#"1 ICON "{ico}"
1 VERSIONINFO
FILEVERSION {commas}
PRODUCTVERSION {commas}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Kiri"
      VALUE "FileDescription", "Kiri"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "kiri-win"
      VALUE "OriginalFilename", "kiri-win.exe"
      VALUE "ProductName", "Kiri"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("kiri-win.rc");
    std::fs::write(&out, rc).expect("write kiri-win.rc");
    embed_resource::compile(&out, embed_resource::NONE).manifest_optional().expect("kiri-win.rc");
}
