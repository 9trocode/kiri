#!/bin/sh
# Kiri Linux, the WSL2 distro Kiri runs in on Windows: Alpine's minirootfs
# (pinned, checked), the static kiri binaries and contrib/wsl/kiri-linux.
# Runs as an ordinary user in WSL; the archive's files are owned by root.
# x86_64 for x86-64 PCs, aarch64 for Windows on Arm (the binaries in <dir>
# must be the same architecture). SHA-256s are Alpine's own .sha256 files.
#   scripts/kiri-linux.sh <repo> <dir with kiri and kiri-init> <out.tar.gz> [x86_64|aarch64]
set -eu
repo=$1 bin=$2 out=$3 arch=${4:-x86_64}
case "$arch" in
  x86_64) ALPINE_SHA256=f8e1cafba9b1b283b5d30d06f9885c8d2bc161ffe9083ade8e2e157813a53407 ;;
  aarch64) ALPINE_SHA256=4d3f80afaebd447c5591a63a73bde380a149fe8b2d85488cafabb2ff97551f50 ;;
  *) echo "arch is x86_64 or aarch64, not $arch" >&2; exit 2 ;;
esac
ALPINE=alpine-minirootfs-3.24.3-$arch.tar.gz
ALPINE_URL=https://dl-cdn.alpinelinux.org/alpine/v3.24/releases/$arch/$ALPINE
cache=${XDG_CACHE_HOME:-$HOME/.cache}/kiri-build
mkdir -p "$cache"
# The .part is this process's own: CI jobs on one machine share the cache.
[ -f "$cache/$ALPINE" ] || { curl -fsSL --retry 3 -o "$cache/$ALPINE.$$.part" "$ALPINE_URL" && mv -f "$cache/$ALPINE.$$.part" "$cache/$ALPINE"; } || rm -f "$cache/$ALPINE.$$.part"
echo "$ALPINE_SHA256  $cache/$ALPINE" | sha256sum -c - >/dev/null || { rm -f "$cache/$ALPINE"; echo "$ALPINE: SHA-256 mismatch" >&2; exit 1; }
root=$(mktemp -d)
trap 'rm -rf "$root"' EXIT
tar --no-same-owner -xzf "$cache/$ALPINE" -C "$root"
install -m 755 "$bin/kiri" "$bin/kiri-init" "$repo/contrib/wsl/kiri-linux/kiri-start" "$root/usr/local/bin/"
install -m 644 "$repo/contrib/wsl/kiri-linux/wsl.conf" "$root/etc/wsl.conf"
tar -C "$root" --numeric-owner --owner=0 --group=0 -czf "$out" .
