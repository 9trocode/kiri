#!/bin/sh
# Puts the guest kernel and the empty data disk (data.img) in guest/out,
# from a local cache when what they're built from hasn't changed,
# otherwise by building them (cargo xtask kernel / disk, which need a docker
# engine) and caching the result. arm64 (default): guest/out/Image, for
# Apple silicon. x86_64: guest/out/x86_64/bzImage, for Intel Macs
# (cross-compiled here).
#   scripts/ci/guest-cache.sh [arm64|x86_64]
set -eu
arch=${1:-arm64}
cache="${KIRI_CI_CACHE:-$HOME/Library/Caches/kiri-ci}"
mkdir -p "$cache" guest/out

case "$arch" in
  arm64) kfile=Image kout=guest/out extra="" ;;
  x86_64) kfile=bzImage kout=guest/out/x86_64 extra=guest/kernel/kiri-x86_64.config ;;
  *) echo "arch is arm64 or x86_64, not $arch" >&2; exit 2 ;;
esac
mkdir -p "$kout"
kernel_key=$(cat guest/kernel/build.sh guest/kernel/kiri.config $extra guest/kernel/patches/*.patch 2>/dev/null | shasum -a 256 | cut -c1-16)
if [ -f "$cache/$kfile-$kernel_key" ]; then
  echo "kernel: cached ($kfile, $kernel_key)"
else
  echo "kernel: building ($kfile, $kernel_key)"
  cargo xtask kernel --arch "$arch"
  cp "$kout/$kfile" "$cache/$kfile-$kernel_key"
fi
cp "$cache/$kfile-$kernel_key" "$kout/$kfile"

# The disk is a blank, sparse ext4 image; its recipe lives in xtask's disk().
disk_key=$(awk '/^fn disk\(/,/^}/' xtask/src/main.rs | shasum -a 256 | cut -c1-16)
if [ -f "$cache/data-$disk_key.img" ]; then
  echo "disk: cached ($disk_key)"
else
  echo "disk: building ($disk_key)"
  cargo xtask disk
  cp -c guest/out/data.img "$cache/data-$disk_key.img" 2>/dev/null || cp guest/out/data.img "$cache/data-$disk_key.img"
fi
# An APFS clone: instant, and the sparse 64 GiB stays a few MB.
rm -f guest/out/data.img
cp -c "$cache/data-$disk_key.img" guest/out/data.img 2>/dev/null || cp "$cache/data-$disk_key.img" guest/out/data.img
