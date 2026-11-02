//! PID 1 of the Kiri guest VM, and its agent.
//!
//! It mounts the kernel filesystems, dials the host over vsock, reports how
//! long boot took, then serves requests until told to power off. If the host
//! goes away the VM powers itself off, so a crashed host never leaves an
//! orphan VM holding memory.

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
fn main() {
    linux::main();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("kiri-init only runs inside the Linux guest");
    std::process::exit(1);
}
