#!/bin/bash
# Root: start every competitor engine on its own socket, nothing enabled as a service.
# The everyday (non-root) user who runs Colima and Lima: run with sudo, or set BENCH_USER.
BENCH_USER=${BENCH_USER:-${SUDO_USER:?run with sudo or set BENCH_USER}}
export DEBIAN_FRONTEND=noninteractive
apt-get -qq install -y iptables uidmap >/dev/null 2>&1
usermod -aG kvm "$BENCH_USER"
printf 'docker-engine %s\nnerdctl %s\ncontainerd %s\npodman %s\ncrun %s\ncolima %s\nlima %s\nhyperfine %s\nkernel %s\n' \
  "$(/opt/docker-ce/dockerd --version | cut -d' ' -f3 | tr -d ,)" "$(/opt/nerdctl/bin/nerdctl --version | cut -d' ' -f3)" "$(/opt/nerdctl/bin/containerd --version | cut -d' ' -f3)" \
  "$(podman --version | cut -d' ' -f3)" "$(crun --version | head -1 | cut -d' ' -f3)" "$(colima version 2>/dev/null | head -1 | cut -d' ' -f3)" "$(su - "$BENCH_USER" -c 'limactl --version' | cut -d' ' -f3)" "$(hyperfine --version | cut -d' ' -f2)" "$(uname -r)" | tee /root/versions.txt
cat > /usr/local/bin/nerdctl <<'W'
#!/bin/sh
exec /opt/nerdctl/bin/nerdctl --cni-path /opt/nerdctl/libexec/cni "$@"
W
chmod +x /usr/local/bin/nerdctl
export PATH=/opt/docker-ce:/opt/nerdctl/bin:$PATH
pgrep -f dockerd-bench >/dev/null || setsid -f /opt/docker-ce/dockerd -H unix:///run/dockerd-bench.sock --data-root /var/lib/docker-bench --pidfile /run/dockerd-bench.pid --exec-root /run/docker-bench >/var/log/dockerd-bench.log 2>&1 </dev/null
pgrep -x containerd >/dev/null || setsid -f /opt/nerdctl/bin/containerd >/var/log/containerd-bench.log 2>&1 </dev/null
pgrep -x buildkitd >/dev/null || setsid -f /opt/nerdctl/bin/buildkitd >/var/log/buildkitd-bench.log 2>&1 </dev/null
pgrep -f 'podman system service' >/dev/null || setsid -f podman system service --time=0 unix:///run/podman-bench.sock >/var/log/podman-bench.log 2>&1 </dev/null
sleep 4
echo "docker-engine: $(DOCKER_HOST=unix:///run/dockerd-bench.sock docker version --format '{{.Server.Version}}' 2>&1 | head -1)"
echo "podman-api:    $(DOCKER_HOST=unix:///run/podman-bench.sock docker version --format '{{.Server.Version}}' 2>&1 | head -1)"
echo "nerdctl:       $(nerdctl version --format '{{.Server.Components}}' 2>&1 | head -c 80)"
echo "kiri:          $(DOCKER_HOST=unix:///var/lib/kiri-daemon/.kiri/run/docker.sock docker version --format '{{.Server.Version}}' 2>&1 | head -1)"
echo "desktop:       $(docker version --format '{{.Server.Version}}' 2>&1 | head -1)"
su - "$BENCH_USER" -c "setsid -f bash -c 'PATH=/usr/local/bin:/opt/lima/bin:\$PATH colima start --runtime docker --cpu 8 --memory 8 --disk 30 --vm-type qemu > ~/colima-start.log 2>&1; echo EXIT=\$? >> ~/colima-start.log' </dev/null >/dev/null 2>&1"
echo "colima: starting in background"
