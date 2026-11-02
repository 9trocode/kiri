#!/bin/sh
# Kiri's eight Linux packages from this checkout, on any Debian or Ubuntu
# with Rust, python3-venv, rpm and dpkg-deb: the release build's Ubuntu, WSL's
# Ubuntu on a Windows PC, or the container linux-packages.sh starts on a Mac.
# - the static engine for x86-64 and arm64 (scripts/windows-engine.sh, zig
#   cross-compiling)
# - the docker CLI, Compose and buildx at their pins
#   (packaging/linux/fetch-cli.sh), kept in target/linux-cli-<arch> for the
#   host test (packaging/linux/test-host.sh)
# - the window (kiri --features window, against this machine's glibc, GTK
#   and WebKitGTK) for this machine's own architecture, when
#   libwebkit2gtk-4.1-dev is installed; it goes in the full kiri package
# - kiri and kiri-lite as .deb and .rpm for each (packaging/linux/build.sh)
#
#   scripts/ci/linux-build.sh
#
# Leaves them and SHA256SUMS-linux in target/packages. zig, the crates and
# the build stay in ~/.cache between runs.
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
rm -rf target/packages
mkdir -p target/packages
# The other architecture's .rpm is built here too: its contents are static
# binaries, so nothing is compiled for it, but rpmbuild has to be told it may.
grep -qs "^buildarch_compat: $(uname -m):" "$HOME/.rpmrc" || echo "buildarch_compat: $(uname -m): x86_64 aarch64 noarch" >> "$HOME/.rpmrc"
bins=$(mktemp -d)
trap 'rm -rf "$bins"' EXIT
# Its own target dir: the same crate with another feature set would otherwise
# rebuild over the engine's build each time. No --locked, as for the engine:
# a test build sets Cargo.toml to <release>-dev.N, which changes the lock file.
window=
if pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  # In ~/.cache, as the engine's is (scripts/windows-engine.sh): under WSL a
  # checkout on C: is slow for cargo's many small files.
  wtarget=$HOME/.cache/kiri-window-target/$(printf %s "$PWD" | sha256sum | cut -c1-12)
  CARGO_TARGET_DIR=$wtarget cargo build --release -p kiri --features window
  window=$wtarget/release/kiri
else
  echo "no libwebkit2gtk-4.1-dev: the packages have no window" >&2
fi
for arch in x86_64 aarch64; do
  bash scripts/windows-engine.sh "$root" "$bins/$arch" "$arch"
  bash packaging/linux/fetch-cli.sh "target/linux-cli-$arch" "$HOME/.cache/kiri-linux-cli" "$arch"
  w=; [ "$arch" = "$(uname -m)" ] && w=$window
  bash packaging/linux/build.sh "$bins/$arch" target/packages "$arch" "target/linux-cli-$arch" $w
done
cd target/packages
sha256sum *.deb *.rpm > SHA256SUMS-linux
cat SHA256SUMS-linux
