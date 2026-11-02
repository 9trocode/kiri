#!/bin/bash
# The docker CLI, Compose and buildx the full `kiri` package bundles, at the
# versions and SHA-256s pinned in crates/kiri/src/cli_pins.rs (LINUX_* for
# x86_64, LINUX_ARM64_* for aarch64):
#   packaging/linux/fetch-cli.sh <cli-dir> [cache-dir] [x86_64|aarch64]
# Leaves <cli-dir>/docker, docker-compose, docker-buildx, each verified.
set -euo pipefail
dir=$1 cache=${2:-${XDG_CACHE_HOME:-$HOME/.cache}/kiri-linux-cli} arch=${3:-x86_64}
# The pins' prefix, and each project's name for the architecture.
case $arch in
  x86_64) p=LINUX_ compose_arch=x86_64 go_arch=amd64 ;;
  aarch64) p=LINUX_ARM64_ compose_arch=aarch64 go_arch=arm64 ;;
  *) echo "arch must be x86_64 or aarch64" >&2; exit 1 ;;
esac
root=$(cd "$(dirname "$0")/../.." && pwd)
pins=$root/crates/kiri/src/cli_pins.rs
pin() { sed -n "s/^pub const $1: (&str, &str) = (\"\([^\"]*\)\", \"\([0-9a-f]*\)\");.*/\1 \2/p" "$pins" | tr -d '\r'; }
read -r dv dsum <<<"$(pin ${p}DOCKER_CLI)"
read -r cv csum <<<"$(pin ${p}COMPOSE)"
read -r bv bsum <<<"$(pin ${p}BUILDX)"
[ -n "$dv" ] && [ -n "$cv" ] && [ -n "$bv" ] || { echo "no ${p}* pins in $pins" >&2; exit 1; }
mkdir -p "$dir" "$cache"
dir=$(cd "$dir" && pwd)
get() { # url file sum
  local f=$cache/$2
  if [ ! -f "$f" ]; then
    # HTTPS only, redirects included; a failed download leaves no .part behind.
    # The .part is this process's own, as CI jobs on one machine share the cache.
    if ! curl -fsSL --proto '=https' --proto-redir '=https' --retry 3 -o "$f.$$.part" "$1"; then
      rm -f "$f.$$.part"
      echo "could not download $1" >&2
      exit 1
    fi
    mv "$f.$$.part" "$f"
  fi
  echo "$3  $f" | sha256sum -c --quiet - || { echo "$2 does not match the pinned SHA-256; delete $f and retry" >&2; exit 1; }
}
get "https://download.docker.com/linux/static/stable/$arch/docker-$dv.tgz" "docker-$dv-$arch.tgz" "$dsum"
get "https://github.com/docker/compose/releases/download/$cv/docker-compose-linux-$compose_arch" "docker-compose-$cv-$arch" "$csum"
get "https://github.com/docker/buildx/releases/download/$bv/buildx-$bv.linux-$go_arch" "buildx-$bv-$arch" "$bsum"
tar xzf "$cache/docker-$dv-$arch.tgz" -C "$dir" --strip-components=1 docker/docker
install -m 755 "$cache/docker-compose-$cv-$arch" "$dir/docker-compose"
install -m 755 "$cache/buildx-$bv-$arch" "$dir/docker-buildx"
echo "$dir: docker $dv, compose $cv, buildx $bv ($arch)"
