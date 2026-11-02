# Testing Kiri for Windows on Arm

Kiri for Windows on Arm is the same bundle and MSI as the x86-64 one, built
with `--arch arm64`. It is arm64 all the way down: `kiri-win.exe` for
`aarch64-pc-windows-msvc`, the engine and Kiri Linux for aarch64 Linux in
WSL2, and an arm64 `docker.exe`, Compose and buildx. This page covers how to
build it, and how much of it a Windows 11 VM on an Apple silicon Mac can test.

## The short answer on VMs

**WSL2 does not run in a Windows 11 on Arm VM on a Mac, on any of the three
hypervisors, as of October 2026.** WSL2 is a Hyper-V VM. Windows needs the
CPU's EL2 (hypervisor) level to run one, so the Mac's hypervisor has to
offer nested virtualization to a Windows guest. None does:

| Hypervisor | Nested virtualization for a Windows on Arm guest | Source |
| --- | --- | --- |
| Parallels Desktop (26, 27) | No. "Nested Virtualization is not available ... WSL2 also will not work; use WSL1 as a workaround" (KB last reviewed 25 Aug 2026) | [KB 129497](https://kb.parallels.com/en/129497), [KB 129234](https://kb.parallels.com/en/129234) |
| UTM | Linux guests only, on the Apple Virtualization backend (not QEMU), M3 or later and macOS 15 or later (UTM 4.6.0+). Nobody reports Windows with Hyper-V or WSL2 working, and Windows does not run on the Apple Virtualization backend anyway | [UTM #6700](https://github.com/utmapp/UTM/issues/6700) |
| VMware Fusion | No. Broadcom (Jan 2025): not available on M-series Macs, no date given. Fusion uses Hypervisor.framework, not Virtualization.framework | [Broadcom community](https://community.broadcom.com/vmware-cloud-foundation/question/nested-virtualization-with-vmware-fusion-in-sequoia) |

Apple's side: Virtualization.framework added nested virtualization in
macOS 15, on M3 and later, for Linux guests only (and it does not run
Windows). Hypervisor.framework has had an EL2 switch since macOS 15
([`hv_vm_config_set_el2_enabled`](https://developer.apple.com/documentation/hypervisor/hv_vm_config_set_el2_enabled(_:_:)),
[`hv_vm_config_get_el2_supported`](https://developer.apple.com/documentation/hypervisor/hv_vm_config_get_el2_supported(_:))).
That is the switch Parallels, Fusion or QEMU would need to turn on for a
Windows guest, and none of them has. Getting a vCPU to actually start in EL2
takes more than that call
([Apple forums](https://developer.apple.com/forums/thread/775546)). So
an M3 or later Mac has the hardware, but no shipping product uses it for Windows.

WSL1 runs in these VMs, but it is no use for Kiri. It translates Linux
syscalls and has no real kernel, so it has no namespaces, cgroups or
overlayfs for the engine.

Cloud Arm VMs do not help either. Azure's Cobalt 100 sizes list nested
virtualization as not supported
([Dplsv6](https://learn.microsoft.com/azure/virtual-machines/sizes/general-purpose/dplsv6-series)).

**For an end-to-end test (`docker run` on Windows on Arm), use real Arm
hardware:** a Snapdragon X laptop (Copilot+ PC), or a Windows Dev Kit 2023.

## What a Mac VM does test

Everything up to the point where WSL2 starts:

- the arm64 MSI: it installs per user, has an Arm64 Platform, and
  upgrades and uninstalls cleanly (`installer/windows/verify.ps1`)
- `kiri-win.exe` runs natively (Task Manager, Details, Architecture
  column: ARM64), with its window, tray icon and named pipe
- the bundled `docker.exe`, Compose and buildx run natively
  (`docker version`, `docker compose version`, `docker buildx version`)
- first-run setup: it downloads the **arm64** WSL MSI, checks its SHA-256
  and Microsoft signature, and turns on Virtual Machine Platform. After the
  restart, WSL2 fails to start, and that failure is expected in a VM
- the architecture guard: the x64 MSI installs on the Arm VM and runs under
  emulation, and its setup stops with "This is Kiri for x64 PCs, and this PC
  is arm64 ... Install Kiri-<version>-arm64.msi instead."
- installing the arm64 MSI over the x64 one: it replaces it (shared
  UpgradeCode, major upgrade)
- the updater reads `latest-windows-arm64.json`, and refuses an x64 MSI
  (platform check)

## Setting up the VM (Parallels or UTM)

Parallels is the quickest: it downloads and activates Windows 11 on Arm for
you. UTM is free, and the QEMU backend is fine here because nested
virtualization is not on offer either way. VMware Fusion (free for personal
use) works the same way.

### Parallels Desktop

1. Install Parallels Desktop.
2. File > New > "Get Windows 11 from Microsoft" > Continue. Parallels
   downloads Windows 11 on Arm and installs it. Give it 4 CPUs, 8 GB of
   memory and a 64 GB disk.
3. In Windows: Settings > Windows Update, and install everything.
4. Check the architecture: in PowerShell, `$env:PROCESSOR_ARCHITECTURE`
   prints `ARM64`.
5. Take a snapshot ("clean"), so each MSI test starts from the same state.

### UTM

1. Install UTM (`brew install --cask utm`, or from mac.getutm.app).
2. Get the Windows 11 on Arm ISO: Microsoft's "Download Windows 11 Arm64"
   page (microsoft.com/software-download/windows11arm64), or CrystalFetch,
   which builds one from Microsoft's servers.
3. UTM > Create a New Virtual Machine > Virtualize > Windows. Choose the
   ISO, keep "Install drivers and SPICE tools", 4 CPUs, 8 GB of memory and
   a 64 GB disk.
4. Install Windows. If setup insists on a network before the drivers are
   in: Shift+F10, then `OOBE\BYPASSNRO`. Then install the SPICE guest tools
   from the drive UTM attaches.
5. Run Windows Update, check `$env:PROCESSOR_ARCHITECTURE` is `ARM64`, and
   clone or snapshot the VM.

### In the VM, for either one

- WebView2 is part of Windows 11, so nothing else is needed for Kiri's
  window.
- Copy the MSIs in through the shared folder (Parallels: the Mac's home is
  `\\Mac\Home`; UTM: SPICE WebDAV or a shared directory).

## Building the arm64 MSI

`cargo xtask msi --arch arm64` runs on Windows, like the x86-64 one. The
Linux half is built in WSL, so build on a PC with working WSL2 (an x86-64
Windows PC, or real Arm hardware), **not** inside a Mac VM. You need:

1. Everything the x86-64 build needs: Rust (MSVC), Visual Studio Build
   Tools, WSL with Rust installed in the default distro, and WiX 5:

       dotnet tool install -g wix
       wix extension add -g WixToolset.UI.wixext/5.0.2
       wix extension add -g WixToolset.Util.wixext/5.0.2

2. For an arm64 kiri-win from an x86-64 PC, the arm64 compiler:
   `rustup target add aarch64-pc-windows-msvc`, and in the Visual Studio
   Installer, Individual components, "MSVC v143 - VS 2022 C++ ARM64/ARM64EC
   build tools (Latest)". ring needs clang for aarch64 Windows, so also add
   "C++ Clang Compiler for Windows" and put its `bin` on PATH (ring's build
   calls `clang` for this target). On an Arm PC, kiri-win builds natively
   and none of this applies.
3. `git` and a Go of 1.21 or later on PATH, for docker.exe (below). Go
   fetches the pinned toolchain (go1.26.10) by itself.
4. Then:

       cargo xtask msi --arch arm64

   which makes `target\kiri-windows-aarch64\`,
   `target\Kiri-<version>-arm64.msi` and `target\latest-windows-arm64.json`.
   `cargo xtask windows --arch arm64` makes only the bundle.
   `cargo xtask msi-pack --arch arm64` repacks a bundle that is already
   there, and works inside the Mac VM too if you copy the bundle in
   (WiX runs on Windows on Arm).

To check the arm64 kiri-win from the Mac without Windows (cargo-xwin, with
LLVM on PATH):

    PATH="$(ls -d /opt/homebrew/Cellar/llvm@22/*/bin):$PATH" \
      cargo xwin check --cross-compiler clang -p kiri-win --target aarch64-pc-windows-msvc

`--cross-compiler clang` matters: ring builds aarch64 Windows code with
`clang`, and xwin's default clang-cl flags (`/imsvc`) break it.

## Why docker.exe is built from source

Docker publishes no arm64 Windows `docker.exe`:
`download.docker.com/win/static/stable/` has `x86_64/` only. Compose and
buildx do publish arm64 Windows builds, and those are pinned by GitHub's
digests like the x86-64 ones.

`cargo xtask docker-win-arm64` (also run by `windows --arch arm64`) builds
docker/cli at the commit the v29.9.2 tag points to. docker/cli's own
release matrix includes `windows/arm64` (`docker-bake.hcl`, `_platforms`).
The build uses:

- the vendored dependencies in that commit (`-mod=vendor`). The checkout
  must be exactly the commit, apart from the go.mod and go.sum copies.
- Go go1.26.10, the version docker/cli's Dockerfile names, through
  `GOTOOLCHAIN`, which `go` downloads and checks against sum.golang.org
- `CGO_ENABLED=0`, `-trimpath`, `-buildvcs=false`, an empty build id, and
  the commit's own time as BuildTime

Go builds like this are reproducible, so the resulting `docker.exe` is
pinned by SHA-256 in `crates/kiri/src/cli_pins.rs`, and a build that comes
out different fails. Two clean builds (a fresh clone and an empty Go cache)
gave the same bytes. A build on another host OS should
match too, under Go's reproducibility guarantee
([go.dev/blog/rebuild](https://go.dev/blog/rebuild)). The first build on
Windows or WSL confirms it.

This build differs from Docker's own Windows binaries in two ways. It has
no cgo, which the Windows CLI does not use; netgo is off anyway, so names
resolve through Windows' resolver either way. It also has no Windows
version resource (icon and file details), which `goversioninfo` would add.

The other option was to ship the x86-64 `docker.exe` under Windows' x64
emulation (Prism). It works, but every `docker` command would pay
emulation start-up, and it would be the bundle's only emulated binary.

## Testing on real Arm hardware (end to end)

After installing `Kiri-<version>-arm64.msi` and finishing setup with Kiri
Linux:

    docker version                       # Server: Os linux, Arch arm64
    docker run --rm alpine uname -m      # aarch64
    docker run --rm --platform linux/amd64 alpine uname -m
                                         # needs amd64 emulation in WSL, which
                                         # this build does not add
    docker compose version; docker buildx version
    wsl -d kiri -- uname -m              # aarch64

Then do the same with the Ubuntu choice (arm64 Ubuntu 26.04 from
cdimages.ubuntu.com, SHA-256 from Microsoft's DistributionInfo.json).
