#!/bin/bash
# Kiri for Windows, without Windows: copies target/kiri-windows-x86_64 (what
# `cargo xtask windows` makes) into an x86-64 Ubuntu container standing in
# for the WSL2 distro, runs its install.sh as a Windows user would, then
# drives the engine from a second container with only the docker CLI,
# standing in for Windows. The daemon serves only its unix socket (on
# Windows, kiri-win.exe reaches it through `kiri bridge`), so the second
# container shares the socket's directory. On Apple Silicon use aarch64:
# Rosetta runs the x86-64 build but has no clone3, which every container
# start needs (WSL2's kernel has it). What this cannot cover: WSL2's own
# kernel, and kiri-win.exe's relay. Usage: tests/wsl.sh [x86_64|aarch64]
set -u
D=${D:-docker}
ARCH=${1:-x86_64}
case $ARCH in x86_64) PLATFORM=linux/amd64 ;; aarch64) PLATFORM=linux/arm64 ;; *) echo "usage: $0 [x86_64|aarch64]"; exit 2 ;; esac
DIR=$(cd "$(dirname "$0")/.." && pwd)/target/kiri-windows-$ARCH
[ -x "$DIR/kiri" ] || { echo "build first: cargo xtask windows (--arch arm64 for aarch64)"; exit 1; }
SOCK=/var/lib/kiri-daemon/.kiri/run
$D rm -f kiri-wsl kiri-wsl-cli >/dev/null 2>&1
$D volume rm kiri-wsl-run kiri-wsl-lib kiri-wsl-store >/dev/null 2>&1
# WSL's disk is ext4; this container's own root is overlayfs, which can't be
# an overlay's upper layer, so Kiri's state goes on volumes, as in
# linux-host.sh.
$D run -d --name kiri-wsl --platform $PLATFORM --privileged --tmpfs /run -v kiri-wsl-run:$SOCK \
  -v kiri-wsl-lib:/var/lib/kiri -v kiri-wsl-store:/var/lib/kiri-daemon/.local/share/kiri ubuntu:24.04 sleep infinity >/dev/null
$D exec kiri-wsl mkdir -p /tmp/kiri-windows
$D cp "$DIR/kiri" kiri-wsl:/tmp/kiri-windows/kiri >/dev/null
$D cp "$DIR/kiri-init" kiri-wsl:/tmp/kiri-windows/kiri-init >/dev/null
$D cp "$DIR/install.sh" kiri-wsl:/tmp/kiri-windows/install.sh >/dev/null
$D exec kiri-wsl sh -c 'cd /tmp/kiri-windows && sh ./install.sh'
$D run -d --name kiri-wsl-cli --network container:kiri-wsl -v kiri-wsl-run:/kiri alpine sh -c 'apk add -q docker-cli curl >/dev/null 2>&1; sleep infinity' >/dev/null
for _ in $(seq 90); do $D exec kiri-wsl-cli docker -H unix:///kiri/docker.sock version >/dev/null 2>&1 && break; sleep 1; done
PASS=0; FAIL=0
t() {
  local name=$1 want=$2; shift 2
  local got; got=$($D exec kiri-wsl-cli sh -c "export DOCKER_HOST=unix:///kiri/docker.sock; $*" 2>&1 | tr -d '\r')
  if [[ "$got" == *"$want"* ]]; then PASS=$((PASS + 1)); else FAIL=$((FAIL + 1)); printf '  FAIL %s\n    want: %s\n    got:  %s\n' "$name" "$want" "$(echo "$got" | head -3)"; fi
}
# No systemd here, so install.sh must have added the WSL boot command.
if $D exec kiri-wsl grep -q '/usr/local/bin/kiri daemon' /etc/wsl.conf; then PASS=$((PASS + 1)); else FAIL=$((FAIL + 1)); echo "  FAIL boot-entry: no kiri line in /etc/wsl.conf"; fi
t arch "$ARCH" "docker version --format '{{.Server.Arch}}' | sed 's/amd64/x86_64/; s/arm64/aarch64/'"
t hello "Hello from Docker!" "docker run --rm hello-world"
t run "3." "docker run --rm alpine cat /etc/alpine-release"
t internet "Example Domain" "docker run --rm alpine wget -qO- -T 10 http://example.com"
t port "Welcome to nginx" "docker run -d --name web -p 18082:80 nginx:alpine >/dev/null; for i in \$(seq 40); do curl -s localhost:18082 && break; sleep 0.5; done"
t exec "nginx version" "docker exec web nginx -v"
t network-dns "PONG" "docker network create wn >/dev/null 2>&1; docker run -d --name db --network wn redis:7-alpine >/dev/null; sleep 2; docker run --rm --network wn redis:7-alpine redis-cli -h db ping"
t volume "kept" "docker run --rm -v wv:/d alpine sh -c 'echo kept > /d/f' && docker run --rm -v wv:/d alpine cat /d/f"
t seccomp "Seccomp:	2" "docker run --rm alpine grep Seccomp: /proc/self/status"
t cleanup "web" "docker rm -f web db"
$D rm -f kiri-wsl kiri-wsl-cli >/dev/null
$D volume rm kiri-wsl-run kiri-wsl-lib kiri-wsl-store >/dev/null 2>&1
echo "$PASS passed, $FAIL failed"
[ "$FAIL" = 0 ]
