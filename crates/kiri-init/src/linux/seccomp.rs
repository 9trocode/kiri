//! The default syscall filter, after Docker's: syscalls that reach outside
//! the container's namespaces (kernel keyrings, modules, the clock, swap,
//! new mount and user namespaces, io_uring, bpf) fail with EPERM unless the
//! container holds the capability that would make them legitimate. A deny
//! list rather than Docker's allow list: the same protection for a fraction
//! of the program length, so every syscall pays fewer instructions.

use std::io;

/// The architecture syscalls must come from. On Apple silicon Rosetta
/// translates x86-64 binaries to arm64 syscalls, so arm64 is the only one.
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xC000_00B7; // AUDIT_ARCH_AARCH64
#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xC000_003E; // AUDIT_ARCH_X86_64

const RET_ALLOW: u32 = 0x7fff_0000;
const RET_ERRNO: u32 = 0x0005_0000;

const LD_W_ABS: u16 = 0x20;
const JEQ_K: u16 = 0x15;
const JSET_K: u16 = 0x45;
#[cfg(target_arch = "x86_64")]
const JGE_K: u16 = 0x35;
const RET_K: u16 = 0x06;

/// Without it the kernel turns on the Speculative Store Bypass mitigation
/// for every filtered task, which in a VM is a firmware call on each
/// context switch: half a millisecond on a short-lived container. That
/// mitigation guards a process against itself (JIT sandboxes), not
/// containers against each other, which separate processes already are.
const SECCOMP_FILTER_FLAG_SPEC_ALLOW: libc::c_ulong = 1 << 2;

/// Not in every libc list.
#[cfg(target_arch = "aarch64")]
const SYS_KEXEC_FILE_LOAD: libc::c_long = 294;
#[cfg(target_arch = "x86_64")]
const SYS_KEXEC_FILE_LOAD: libc::c_long = 320;

/// x86-64's x32 ABI numbers its syscalls from here; they would otherwise
/// slip past every rule.
#[cfg(target_arch = "x86_64")]
const X32_SYSCALL_BIT: u32 = 0x4000_0000;

/// seccomp_data offsets.
const OFF_NR: u32 = 0;
const OFF_ARCH: u32 = 4;
const OFF_ARG0: u32 = 16;

// Capability numbers that relax the filter, as in Docker's profile.
const CAP_SYS_MODULE: u32 = 16;
const CAP_SYS_PACCT: u32 = 20;
const CAP_SYS_ADMIN: u32 = 21;
const CAP_SYS_BOOT: u32 = 22;
const CAP_SYS_NICE: u32 = 23;
const CAP_SYS_TIME: u32 = 25;
const CAP_PERFMON: u32 = 38;
const CAP_BPF: u32 = 39;

const NS_FLAGS: u32 = (libc::CLONE_NEWNS
    | libc::CLONE_NEWUTS
    | libc::CLONE_NEWIPC
    | libc::CLONE_NEWUSER
    | libc::CLONE_NEWPID
    | libc::CLONE_NEWNET
    | libc::CLONE_NEWCGROUP) as u32;

#[repr(C)]
#[derive(Clone, Copy)]
struct Insn {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
struct Prog {
    len: u16,
    filter: *const Insn,
}

const fn stmt(code: u16, k: u32) -> Insn {
    Insn { code, jt: 0, jf: 0, k }
}

const fn jump(code: u16, k: u32, jt: u8, jf: u8) -> Insn {
    Insn { code, jt, jf, k }
}

/// Denied syscalls, each with the capability that would allow it (or none).
fn denied() -> Vec<(libc::c_long, Option<u32>)> {
    use libc::*;
    vec![
        // Kernel keyrings are not namespaced.
        (SYS_add_key, None),
        (SYS_keyctl, None),
        // Kernel log and KASLR hints; Docker allows it only with CAP_SYSLOG.
        (libc::SYS_syslog, None),
        (SYS_request_key, None),
        // Modules and kexec.
        (SYS_init_module, Some(CAP_SYS_MODULE)),
        (SYS_finit_module, Some(CAP_SYS_MODULE)),
        (SYS_delete_module, Some(CAP_SYS_MODULE)),
        (SYS_kexec_load, Some(CAP_SYS_BOOT)),
        (SYS_KEXEC_FILE_LOAD, Some(CAP_SYS_BOOT)),
        (SYS_reboot, Some(CAP_SYS_BOOT)),
        // The VM's clock and accounting.
        (SYS_clock_settime, Some(CAP_SYS_TIME)),
        (SYS_clock_adjtime, Some(CAP_SYS_TIME)),
        (SYS_settimeofday, Some(CAP_SYS_TIME)),
        (SYS_acct, Some(CAP_SYS_PACCT)),
        // Swap, quotas, NUMA policy.
        (SYS_swapon, Some(CAP_SYS_ADMIN)),
        (SYS_swapoff, Some(CAP_SYS_ADMIN)),
        (SYS_quotactl, Some(CAP_SYS_ADMIN)),
        (SYS_mbind, Some(CAP_SYS_NICE)),
        (SYS_set_mempolicy, Some(CAP_SYS_NICE)),
        (SYS_migrate_pages, Some(CAP_SYS_NICE)),
        (SYS_move_pages, Some(CAP_SYS_NICE)),
        // Mounts and namespaces.
        (SYS_mount, Some(CAP_SYS_ADMIN)),
        (SYS_umount2, Some(CAP_SYS_ADMIN)),
        (SYS_pivot_root, Some(CAP_SYS_ADMIN)),
        (SYS_open_tree, Some(CAP_SYS_ADMIN)),
        (SYS_move_mount, Some(CAP_SYS_ADMIN)),
        (SYS_fsopen, Some(CAP_SYS_ADMIN)),
        (SYS_fsconfig, Some(CAP_SYS_ADMIN)),
        (SYS_fsmount, Some(CAP_SYS_ADMIN)),
        (SYS_fspick, Some(CAP_SYS_ADMIN)),
        (SYS_mount_setattr, Some(CAP_SYS_ADMIN)),
        (SYS_unshare, Some(CAP_SYS_ADMIN)),
        (SYS_setns, Some(CAP_SYS_ADMIN)),
        (SYS_name_to_handle_at, Some(CAP_SYS_ADMIN)),
        (SYS_open_by_handle_at, Some(CAP_SYS_ADMIN)),
        (SYS_lookup_dcookie, Some(CAP_SYS_ADMIN)),
        (SYS_vhangup, Some(CAP_SYS_ADMIN)),
        // Kernel attack surface Docker also closes.
        (SYS_bpf, Some(CAP_BPF)),
        (SYS_perf_event_open, Some(CAP_PERFMON)),
        (SYS_userfaultfd, None),
        (SYS_io_uring_setup, None),
        (SYS_io_uring_enter, None),
        (SYS_io_uring_register, None),
    ]
}

/// Every program starts here: the native architecture only, then the
/// syscall number loaded for the rules that follow.
fn head() -> Vec<Insn> {
    #[cfg_attr(not(target_arch = "x86_64"), allow(unused_mut))]
    let mut p = vec![
        stmt(LD_W_ABS, OFF_ARCH),
        jump(JEQ_K, AUDIT_ARCH, 1, 0),
        stmt(RET_K, RET_ERRNO | libc::EPERM as u32),
        stmt(LD_W_ABS, OFF_NR),
    ];
    #[cfg(target_arch = "x86_64")]
    {
        p.push(jump(JGE_K, X32_SYSCALL_BIT, 0, 1));
        p.push(stmt(RET_K, RET_ERRNO | libc::EPERM as u32));
    }
    p
}

fn deny(p: &mut Vec<Insn>, nr: libc::c_long) {
    p.push(jump(JEQ_K, nr as u32, 0, 1));
    p.push(stmt(RET_K, RET_ERRNO | libc::EPERM as u32));
}

/// vsock is not namespaced: with a socket a container could dial the
/// host's ports (the host.docker.internal relay, the SSH agent, the
/// control channel) past `--network none` and the SSH-agent opt-in.
/// Docker's profile refuses it for everyone; so do we, whatever the
/// capabilities. Leaves the syscall number loaded.
fn deny_vsock(p: &mut Vec<Insn>) {
    p.push(jump(JEQ_K, libc::SYS_socket as u32, 0, 4));
    p.push(stmt(LD_W_ABS, OFF_ARG0));
    p.push(jump(JEQ_K, libc::AF_VSOCK as u32, 0, 1));
    p.push(stmt(RET_K, RET_ERRNO | libc::EAFNOSUPPORT as u32));
    p.push(stmt(LD_W_ABS, OFF_NR));
}

/// The filter for `--privileged` and `seccomp=unconfined`: nothing of the
/// default profile, only the host boundary. io_uring goes too, since its
/// socket operation makes a socket without the syscall a filter sees.
fn boundary() -> Vec<Insn> {
    let mut p = head();
    for nr in [libc::SYS_io_uring_setup, libc::SYS_io_uring_enter, libc::SYS_io_uring_register] {
        deny(&mut p, nr);
    }
    deny_vsock(&mut p);
    p.push(stmt(RET_K, RET_ALLOW));
    p
}

/// Builds the filter for a container that keeps `caps` (one bit each).
fn program(caps: u64) -> Vec<Insn> {
    let has = |cap: u32| caps & (1u64 << cap) != 0;
    let mut p = head();
    for (nr, cap) in denied() {
        if cap.is_some_and(has) {
            continue;
        }
        deny(&mut p, nr);
    }
    deny_vsock(&mut p);
    if !has(CAP_SYS_ADMIN) {
        // clone3 hides its flags in memory a filter cannot read: ENOSYS
        // makes libc fall back to clone, whose flags it can.
        p.push(jump(JEQ_K, libc::SYS_clone3 as u32, 0, 1));
        p.push(stmt(RET_K, RET_ERRNO | libc::ENOSYS as u32));
        p.push(jump(JEQ_K, libc::SYS_clone as u32, 0, 3));
        p.push(stmt(LD_W_ABS, OFF_ARG0));
        p.push(jump(JSET_K, NS_FLAGS, 0, 1));
        p.push(stmt(RET_K, RET_ERRNO | libc::EPERM as u32));
    }
    p.push(stmt(RET_K, RET_ALLOW));
    p
}

/// Installs the default filter on this process, or with `None` only the
/// host boundary; what it execs inherits it. Needs CAP_SYS_ADMIN (or
/// no_new_privs), so it runs before the user switch.
pub fn install(caps: Option<u64>) -> io::Result<()> {
    let insns = caps.map_or_else(boundary, program);
    let prog = Prog { len: insns.len() as u16, filter: insns.as_ptr() };
    // SAFETY: a valid sock_fprog whose instructions outlive the call.
    let ret = unsafe { libc::syscall(libc::SYS_seccomp, 1 /* SECCOMP_SET_MODE_FILTER */, SECCOMP_FILTER_FLAG_SPEC_ALLOW, &prog as *const Prog) };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_relax_the_filter() {
        let plain = program(0).len();
        let admin = program(1 << CAP_SYS_ADMIN).len();
        assert!(admin < plain, "SYS_ADMIN allows mounts and namespaces");
        // Every jump lands inside the program.
        for p in [program(0), program(u64::MAX)] {
            for (i, insn) in p.iter().enumerate() {
                if insn.code == JEQ_K || insn.code == JSET_K {
                    assert!(i + 1 + (insn.jt.max(insn.jf) as usize) < p.len());
                }
            }
            assert_eq!(p.last().map(|i| (i.code, i.k)), Some((RET_K, RET_ALLOW)));
        }
    }

    #[test]
    fn vsock_sockets_are_refused_for_everyone() {
        for p in [program(0), program(u64::MAX), boundary()] {
            // socket(AF_VSOCK, …): the number, then arg0, then EAFNOSUPPORT.
            let at = p.iter().position(|i| i.code == JEQ_K && i.k == libc::SYS_socket as u32).expect("a socket rule");
            assert_eq!((p[at + 1].code, p[at + 1].k), (LD_W_ABS, OFF_ARG0));
            assert_eq!((p[at + 2].code, p[at + 2].k, p[at + 2].jf), (JEQ_K, libc::AF_VSOCK as u32, 1));
            assert_eq!((p[at + 3].code, p[at + 3].k), (RET_K, RET_ERRNO | libc::EAFNOSUPPORT as u32));
            // Other sockets fall through with the syscall number reloaded.
            assert_eq!((p[at + 4].code, p[at + 4].k), (LD_W_ABS, OFF_NR));
            assert_eq!(p[at].jf as usize, 4);
        }
    }

    #[test]
    fn boundary_keeps_only_the_host_boundary() {
        let p = boundary();
        let denies = |nr: libc::c_long| p.windows(2).any(|w| w[0].code == JEQ_K && w[0].k == nr as u32 && w[1].k == RET_ERRNO | libc::EPERM as u32);
        for nr in [libc::SYS_io_uring_setup, libc::SYS_io_uring_enter, libc::SYS_io_uring_register] {
            assert!(denies(nr), "io_uring makes sockets the filter cannot see");
        }
        // Nothing of the default profile: mounts, namespaces, keyrings pass.
        for nr in [libc::SYS_mount, libc::SYS_unshare, libc::SYS_keyctl, libc::SYS_clone3] {
            assert!(!denies(nr), "{nr} filtered");
        }
        assert!(!p.windows(2).any(|w| w[1].k == RET_ERRNO | libc::ENOSYS as u32), "clone3 left alone");
        for (i, insn) in p.iter().enumerate() {
            if insn.code == JEQ_K || insn.code == JSET_K {
                assert!(i + 1 + (insn.jt.max(insn.jf) as usize) < p.len());
            }
        }
        assert_eq!(p.last().map(|i| (i.code, i.k)), Some((RET_K, RET_ALLOW)));
    }
}
