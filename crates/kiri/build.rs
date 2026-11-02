//! Compiles the Kiri Share build key into official releases; the work is
//! in build_share_key.rs, which kiri-win's build script includes too.

include!("build_share_key.rs");

fn main() {
    println!("cargo:rerun-if-changed=build_share_key.rs");
    write_share_build_key();
}
