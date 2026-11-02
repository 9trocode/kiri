#!/bin/sh
# Kiri's Linux packages on a Mac, in an arm64 Linux container (the Mac's
# docker engine is Kiri): scripts/ci/linux-build.sh with what it needs
# installed first. Release builds make them on Ubuntu instead and test them
# there and on Fedora.
#
#   scripts/ci/linux-packages.sh
#
# Leaves them and SHA256SUMS-linux in target/packages. Named volumes keep
# zig, the crates, the downloads and the build between runs.
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
docker run --rm --platform linux/arm64 \
  -v "$root:/src" -w /src \
  -v kiri-linux-cache:/root/.cache \
  -v kiri-linux-cargo:/usr/local/cargo/registry \
  -e HOME=/root \
  rust:1-bookworm sh -euc '
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends python3 python3-venv rpm pkg-config libwebkit2gtk-4.1-dev >/dev/null
    sh scripts/ci/linux-build.sh
  '
