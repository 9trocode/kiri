#!/bin/bash
# Installs the benchmark tools and the competitor engines on Ubuntu 24.04 (run as root).
# The everyday (non-root) user who runs Colima and Lima: run with sudo, or set BENCH_USER.
BENCH_USER=${BENCH_USER:-${SUDO_USER:?run with sudo or set BENCH_USER}}
set -eu
export DEBIAN_FRONTEND=noninteractive
echo "### apt"
apt-get -qq update
apt-get -qq install -y hyperfine fio iperf3 podman crun qemu-system-x86 qemu-utils curl jq >/dev/null
fetch_verified() {
  url=$1 file=$2 sum=$3
  if [ -f "$file" ] && echo "$sum  $file" | sha256sum -c - >/dev/null 2>&1; then return; fi
  rm -f "$file"
  curl -fsSL --retry 3 -o "$file.part" "$url"
  echo "$sum  $file.part" | sha256sum -c - >/dev/null || { rm -f "$file.part"; echo "$file: SHA-256 mismatch" >&2; exit 1; }
  mv "$file.part" "$file"
}

echo "### docker engine (static tarball, no apt, no service)"
mkdir -p /opt/docker-ce && cd /opt/docker-ce
fetch_verified https://download.docker.com/linux/static/stable/x86_64/docker-29.9.2.tgz docker-29.9.2.tgz 9ac225999fd5e863f7694c728ef23d480616b1e5639328fa309befd4243d69eb
tar xzf docker-29.9.2.tgz --strip-components=1
./dockerd --version
echo "### nerdctl-full (containerd, runc, cni)"
mkdir -p /opt/nerdctl && cd /opt/nerdctl
fetch_verified https://github.com/containerd/nerdctl/releases/download/v2.4.1/nerdctl-full-2.4.1-linux-amd64.tar.gz nerdctl-full-2.4.1-linux-amd64.tar.gz ff018f11ac2f1354bf7b3aaaca6e0b52d81139c746a89134744d0b7be3a0e62f
tar xzf nerdctl-full-2.4.1-linux-amd64.tar.gz
./bin/nerdctl --version; ./bin/containerd --version
echo "### lima + colima"
mkdir -p /opt/lima && cd /opt/lima
fetch_verified https://github.com/lima-vm/lima/releases/download/v2.2.1/lima-2.2.1-Linux-x86_64.tar.gz lima-2.2.1-Linux-x86_64.tar.gz b391ac7fcac2b2a5a1628107756463188ded4d18d3104a40a66a8bb6fec01bcb
tar xzf lima-2.2.1-Linux-x86_64.tar.gz
fetch_verified https://github.com/abiosoft/colima/releases/download/v0.10.3/colima-Linux-x86_64 /usr/local/bin/colima 54ea35f4847815d0a1445dbc2bc3dd6469518a38a9e960449e96b044d248d00d
chmod +x /usr/local/bin/colima
ln -sf /opt/lima/bin/limactl /usr/local/bin/limactl
/usr/local/bin/colima version | head -1; limactl --version
usermod -aG kvm "$BENCH_USER"
echo "### versions"
hyperfine --version; fio --version; iperf3 --version | head -1; podman --version; crun --version | head -1; qemu-system-x86_64 --version | head -1
echo "### done"
