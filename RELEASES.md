# How Kiri releases are built

Kiri's source is all here. Releases are built from it by a private
repository that holds only the build workflows. This page says what
that repository does, what it doesn't, and how you can check what you
download.

## Why the builds aren't public

The release builds need our signing keys: Apple's Developer ID for the Mac,
our Windows signing certificate, and the token that uploads to
download.runkiri.com. Public CI runs code from anyone who opens a pull
request, so machines that hold those keys can't be public CI runners. VS Code,
IntelliJ and Chrome are built the same way: open source, released from the
maker's own build machines.

## What a release is

1. A version is tagged here as `vX.Y.Z`, signed by the maintainer.
2. The private release repository checks out **this
   repository at that tag**, with a read-only key. It has no source of its
   own. The one change it makes is the version line in `Cargo.toml`, set
   from the tag.
3. It runs the scripts in this repository, the same ones anyone can read and
   run:
   - Mac: `cargo xtask guest`, `cargo xtask release` and `cargo xtask dmg`
     (the VM's guest, the signed app, the disk image; all in `xtask/`), then
     Apple's notarization (`xcrun notarytool`)
   - Windows: `cargo xtask msi` (the installer, from `installer/windows`)
   - Linux: `scripts/ci/linux-build.sh` (all `.deb` and `.rpm` packages),
     then `packaging/linux/test-host.sh`, which installs them on Ubuntu and
     Fedora and runs containers through them. A build that doesn't install
     and run isn't released.
4. The files are uploaded to download.runkiri.com, and the version goes
   live (update feeds, website) only once the Mac, Windows and Linux builds
   are all up. The upload scripts stay private, since they hold our
   storage and signing setup; they upload and check files, nothing else.

The Rust dependencies come from this repository's `Cargo.lock`. The docker CLI, Compose and buildx are downloaded
at versions and SHA-256s pinned in `packaging/`.

## Where it runs

On dedicated build machines that we control. They never run jobs from this
public repository.

## Checking what you download

- **Mac:** `Kiri.app` is signed with our Developer ID and notarized by Apple.
  Check it with `codesign --verify --deep --strict -v /Applications/Kiri.app`
  and `spctl -a -vv /Applications/Kiri.app`.
- **Linux:** every release has `SHA256SUMS-linux` next to its packages.
  Check with `sha256sum -c --ignore-missing SHA256SUMS-linux`.
- **Windows:** Authenticode signing is being set up. Until it's in place the
  installer is unsigned, and Windows SmartScreen asks before running it.
- **Updates:** Kiri checks every update's SHA-256 against the release feed
  before installing it. On the Mac it also checks that the new app is signed
  by the same Developer ID team as the running one, not just signed by
  someone.

## Building it yourself

Everything that builds a release is in `xtask/`, `scripts/` and `packaging/`, so you can build
the same tag yourself (see `CONTRIBUTING.md`). Builds aren't byte-for-byte
reproducible yet, so your files won't match ours bit for bit. Making the Linux
packages reproducible is planned.

## Reporting a problem

If a download doesn't match its checksum or signature, or you think a release
was tampered with, please report it privately as `SECURITY.md` describes.
