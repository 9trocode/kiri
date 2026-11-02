//! A container's device access, as Docker's default device cgroup is: a
//! cgroup-v2 BPF program (`BPF_PROG_TYPE_CGROUP_DEVICE`) attached to the
//! container's cgroup. Without it a container is real root with `CAP_MKNOD`
//! and could `mknod` a block device and read the host's disks. The filter
//! allows making any node (as runc does) and reading/writing the standard
//! character devices, but denies reading or writing block devices, so a
//! node a container makes for a disk is useless. `--device` and
//! `--device-cgroup-rule` add rules to it.
//!
//! If the kernel cannot attach the filter, the caller drops `CAP_MKNOD`
//! instead (the container's `/dev` has no block nodes to begin with), so
//! device access is never left open.

use std::io;
use std::os::fd::RawFd;

/// Capability number for `CAP_MKNOD`; the fallback drops it.
pub const CAP_MKNOD: u32 = 27;

const BPF_PROG_LOAD: i32 = 5;
const BPF_PROG_ATTACH: i32 = 8;
const BPF_PROG_TYPE_CGROUP_DEVICE: u32 = 15;
const BPF_CGROUP_DEVICE: u32 = 6;

/// One eBPF instruction: `code`, `dst`/`src` (4 bits each), signed `off`, `imm`.
const fn insn(code: u8, dst: u8, src: u8, off: i16, imm: i32) -> u64 {
    (code as u64)
        | ((((src << 4) | (dst & 0xf)) as u64) << 8)
        | (((off as u16) as u64) << 16)
        | (((imm as u32) as u64) << 32)
}

/// Access bits in `bpf_cgroup_dev_ctx.access_type >> 16`.
pub const MKNOD: u32 = 1;
pub const READ: u32 = 2;
pub const WRITE: u32 = 4;

/// One allow rule, as `c 10:229 rwm` reads: `kind` 0 for any, 1 block,
/// 2 char; `None` for `*`; `access` the bits it allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub kind: u32,
    pub major: Option<u32>,
    pub minor: Option<u32>,
    pub access: u32,
}

const fn char_dev(major: u32, minor: Option<u32>) -> Rule {
    Rule { kind: 2, major: Some(major), minor, access: MKNOD | READ | WRITE }
}

/// runc's default allow list, device by device: making any node; null,
/// zero, full, random, urandom (1:3,5,7,8,9), tty, console, ptmx
/// (5:0,1,2), every pty (136:*), net/tun (10:200). Using anything else is
/// denied. (/dev/mem and /dev/port are also kept out by the dropped
/// CAP_SYS_RAWIO.)
const DEFAULT_RULES: [Rule; 11] = [
    Rule { kind: 0, major: None, minor: None, access: MKNOD },
    char_dev(136, None),
    char_dev(1, Some(3)),
    char_dev(1, Some(5)),
    char_dev(1, Some(7)),
    char_dev(1, Some(8)),
    char_dev(1, Some(9)),
    char_dev(5, Some(0)),
    char_dev(5, Some(1)),
    char_dev(5, Some(2)),
    char_dev(10, Some(200)),
];

/// The filter program: the default rules, then `extra` (`--device`,
/// `--device-cgroup-rule`). `ctx` is `struct bpf_cgroup_dev_ctx { u32
/// access_type; u32 major; u32 minor; }`; `access_type` is `(access << 16)
/// | type`. Each rule falls through to the next unless the device matches
/// and every requested access bit is one it allows. Returns 1 to allow, 0
/// to deny.
fn program(extra: &[Rule]) -> Vec<u64> {
    // Opcodes: LDX_W 0x61, MOV64_X 0xbf, AND32 0x54, RSH32 0x74, JEQ_K 0x15,
    // JNE_K 0x55, MOV64_K 0xb7, EXIT 0x95.
    let rules: Vec<&Rule> = DEFAULT_RULES.iter().chain(extra).collect();
    let len = |r: &Rule| usize::from(r.kind != 0) + usize::from(r.major.is_some()) + usize::from(r.minor.is_some()) + 3;
    let total = 6 + rules.iter().map(|r| len(r)).sum::<usize>() + 4;
    let allow = total - 2;
    let mut p = Vec::with_capacity(total);
    p.extend([
        insn(0x61, 2, 1, 0, 0),      // r2 = access_type
        insn(0xbf, 3, 2, 0, 0),      // r3 = r2
        insn(0x54, 3, 0, 0, 0xffff), // r3 &= 0xffff   (device type)
        insn(0x74, 2, 0, 0, 16),     // r2 >>= 16       (access bits)
        insn(0x61, 4, 1, 4, 0),      // r4 = major
        insn(0x61, 5, 1, 8, 0),      // r5 = minor
    ]);
    for r in rules {
        let next = p.len() + len(r);
        let skip = |at: usize| (next - at - 1) as i16;
        if r.kind != 0 {
            p.push(insn(0x55, 3, 0, skip(p.len()), r.kind as i32)); // type differs -> next rule
        }
        if let Some(major) = r.major {
            p.push(insn(0x55, 4, 0, skip(p.len()), major as i32));
        }
        if let Some(minor) = r.minor {
            p.push(insn(0x55, 5, 0, skip(p.len()), minor as i32));
        }
        p.push(insn(0xbf, 6, 2, 0, 0)); // r6 = access bits
        p.push(insn(0x54, 6, 0, 0, (!r.access & 0xffff) as i32)); // r6 &= what this rule does not allow
        p.push(insn(0x15, 6, 0, (allow - p.len() - 1) as i16, 0)); // nothing left -> ALLOW
    }
    p.extend([
        insn(0xb7, 0, 0, 0, 0), // DENY:  r0 = 0
        insn(0x95, 0, 0, 0, 0), //        exit
        insn(0xb7, 0, 0, 0, 1), // ALLOW: r0 = 1
        insn(0x95, 0, 0, 0, 0), //        exit
    ]);
    debug_assert_eq!(p.len(), total);
    p
}

/// `devrule` lines' fields (`c`, `10`, `229`, `rwm`) as a rule.
pub fn parse_rule(kind: &str, major: &str, minor: &str, perms: &str) -> Option<Rule> {
    let number = |n: &str| if n == "*" { Some(None) } else { n.parse().ok().map(Some) };
    let kind = match kind {
        "a" => 0,
        "b" => 1,
        "c" => 2,
        _ => return None,
    };
    // `a` is every device, whatever numbers come with it.
    let (major, minor) = if kind == 0 { (None, None) } else { (number(major)?, number(minor)?) };
    Some(Rule { kind, major, minor, access: access_bits(perms)? })
}

/// `rwm` as access bits.
pub fn access_bits(perms: &str) -> Option<u32> {
    let mut a = 0;
    for c in perms.chars() {
        a |= match c {
            'r' => READ,
            'w' => WRITE,
            'm' => MKNOD,
            _ => return None,
        };
    }
    (a != 0).then_some(a)
}

// bpf(2) attr for BPF_PROG_LOAD; trailing fields of the union are left zero
// (the kernel accepts a short attr whose missing tail is zero).
#[repr(C)]
#[derive(Default)]
struct ProgLoad {
    prog_type: u32,
    insn_cnt: u32,
    insns: u64,
    license: u64,
    log_level: u32,
    log_size: u32,
    log_buf: u64,
    kern_version: u32,
    prog_flags: u32,
    prog_name: [u8; 16],
}

#[repr(C)]
#[derive(Default)]
struct ProgAttach {
    target_fd: u32,
    attach_bpf_fd: u32,
    attach_type: u32,
    attach_flags: u32,
}

/// Attaches the device filter to `cgroup_fd` (a fd on the container's cgroup
/// directory). The container is cloned into that cgroup, so the filter is in
/// force before its first instruction. `extra`: rules beyond the default
/// list. `Err` means the caller should drop `CAP_MKNOD` instead.
pub fn restrict(cgroup_fd: RawFd, extra: &[Rule]) -> io::Result<()> {
    let prog = program(extra);
    let license = *b"GPL\0";
    let attr = ProgLoad {
        prog_type: BPF_PROG_TYPE_CGROUP_DEVICE,
        insn_cnt: prog.len() as u32,
        insns: prog.as_ptr() as u64,
        license: license.as_ptr() as u64,
        ..Default::default()
    };
    // SAFETY: bpf(BPF_PROG_LOAD) reads `attr` for `size` bytes; `insns` and
    // `license` point to buffers that outlive the call.
    let prog_fd = unsafe { libc::syscall(libc::SYS_bpf, BPF_PROG_LOAD, &attr as *const _, std::mem::size_of::<ProgLoad>()) };
    if prog_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a program fd we own; closed below.
    let prog_fd = prog_fd as RawFd;
    let attach = ProgAttach {
        target_fd: cgroup_fd as u32,
        attach_bpf_fd: prog_fd as u32,
        attach_type: BPF_CGROUP_DEVICE,
        // ALLOW_MULTI: a container that manages its own cgroups (Docker in
        // Docker, Kubernetes) may attach its own device filter beneath; every
        // attached program must still allow an access.
        attach_flags: 2,
    };
    // SAFETY: bpf(BPF_PROG_ATTACH) reads `attach` for `size` bytes.
    let r = unsafe { libc::syscall(libc::SYS_bpf, BPF_PROG_ATTACH, &attach as *const _, std::mem::size_of::<ProgAttach>()) };
    // SAFETY: closing a fd we own; the attachment lives on in the cgroup.
    unsafe { libc::close(prog_fd) };
    if r < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the program on one access, interpreting just the opcodes it uses.
    fn run(prog: &[u64], kind: u32, access: u32, major: u32, minor: u32) -> bool {
        let ctx = [(access << 16) | kind, major, minor];
        let mut r = [0u64; 11];
        let mut pc = 0;
        loop {
            let i = prog[pc];
            let (code, dst, src) = (i as u8, ((i >> 8) & 0xf) as usize, ((i >> 12) & 0xf) as usize);
            let (off, imm) = ((i >> 16) as u16 as i16, (i >> 32) as u32 as i32);
            pc += 1;
            match code {
                0x61 => r[dst] = u64::from(ctx[off as usize / 4]),
                0xbf => r[dst] = r[src],
                0x54 => r[dst] = u64::from(r[dst] as u32 & imm as u32),
                0x74 => r[dst] = u64::from((r[dst] as u32) >> imm),
                0xb7 => r[dst] = imm as i64 as u64,
                0x15 if r[dst] == imm as i64 as u64 => pc = (pc as isize + off as isize) as usize,
                0x55 if r[dst] != imm as i64 as u64 => pc = (pc as isize + off as isize) as usize,
                0x15 | 0x55 => {}
                0x95 => return r[0] == 1,
                other => panic!("opcode {other:#x}"),
            }
        }
    }

    #[test]
    fn default_list_and_extra_rules() {
        let p = program(&[]);
        let rw = READ | WRITE;
        assert!(run(&p, 2, rw, 1, 3)); // null
        assert!(run(&p, 2, rw, 136, 7)); // a pty
        assert!(run(&p, 2, rw, 10, 200)); // tun
        assert!(run(&p, 1, MKNOD, 8, 0)); // making a disk node
        assert!(!run(&p, 1, READ, 8, 0)); // reading it
        assert!(!run(&p, 2, rw, 10, 229)); // fuse is not in the default list
        assert!(!run(&p, 2, READ | MKNOD, 1, 1)); // /dev/mem
        let p = program(&[parse_rule("c", "10", "229", "rwm").unwrap(), parse_rule("b", "8", "*", "r").unwrap()]);
        assert!(run(&p, 2, rw, 10, 229));
        assert!(run(&p, 1, READ, 8, 16));
        assert!(!run(&p, 1, rw, 8, 16)); // read only
        assert!(!run(&p, 2, READ, 8, 16)); // char 8:16 is another device
        let all = program(&[parse_rule("a", "*", "*", "rwm").unwrap()]);
        assert!(run(&all, 1, rw, 8, 0));
        assert_eq!(parse_rule("c", "x", "1", "r"), None);
        assert_eq!(access_bits("rwx"), None);
    }
}
