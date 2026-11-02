#!/bin/bash
# Kiri on a Linux host: runs the Linux build (cargo xtask linux) as the host
# engine inside a privileged container on the current Docker engine (Kiri
# works), then drives it with the docker CLI from inside. No VM is involved:
# containers run on that Linux kernel directly.
set -u
D=${D:-docker}
BIN=$(cd "$(dirname "$0")/.." && pwd)/target/linux/release
[ -x "$BIN/kiri" ] || { echo "build first: cargo xtask linux"; exit 1; }
$D rm -f kiri-linux-host >/dev/null 2>&1
$D run -d --name kiri-linux-host --privileged --tmpfs /run -v kiri-linux-host:/var/lib/kiri -v kiri-linux-host-store:/root/.local/share/kiri \
  -v "$BIN:/opt/kiri:ro" alpine sh -c 'apk add -q docker-cli curl >/dev/null 2>&1; exec /opt/kiri/kiri daemon' >/dev/null
for _ in $(seq 60); do $D exec kiri-linux-host sh -c 'DOCKER_HOST=unix:///root/.kiri/run/docker.sock docker version' >/dev/null 2>&1 && break; sleep 1; done
PASS=0; FAIL=0
t() {
  local name=$1 want=$2; shift 2
  local got; got=$($D exec kiri-linux-host sh -c "export DOCKER_HOST=unix:///root/.kiri/run/docker.sock; $*" 2>&1 | tr -d '\r')
  if [[ "$got" == *"$want"* ]]; then PASS=$((PASS + 1)); else FAIL=$((FAIL + 1)); printf '  FAIL %s\n    want: %s\n    got:  %s\n' "$name" "$want" "$(echo "$got" | head -3)"; fi
}
t server-os "linux" "docker version --format '{{.Server.Os}}'"
t run "3." "docker run --rm alpine cat /etc/alpine-release"
t home "/root" "docker run --rm alpine sh -c 'echo \$HOME'"
t internet "Example Domain" "docker run --rm alpine wget -qO- -T 10 http://example.com"
t port "Welcome to nginx" "docker run -d --name web -p 18081:80 nginx:alpine >/dev/null; for i in \$(seq 30); do curl -s localhost:18081 && break; sleep 0.3; done"
t exec "nginx version" "docker exec web nginx -v"
t cp "copied" "echo copied > /tmp/x && docker cp /tmp/x web:/tmp/x && docker exec web cat /tmp/x"
t stats "web" "docker stats --no-stream --format '{{.Name}}'"
t network-dns "PONG" "docker network create lhn >/dev/null 2>&1; docker run -d --name db --network lhn redis:7-alpine >/dev/null; sleep 1; docker run --rm --network lhn redis:7-alpine redis-cli -h db ping"
t bind-host-path "3." "docker run --rm -v /etc:/h:ro alpine cat /h/alpine-release"
t volume "kept" "docker run --rm -v lhv:/d alpine sh -c 'echo kept > /d/f' && docker run --rm -v lhv:/d alpine cat /d/f"
t memory-limit "67108864" "docker run --rm --memory 64m alpine cat /sys/fs/cgroup/memory.max"
t seccomp "Seccomp:	2" "docker run --rm alpine grep Seccomp: /proc/self/status"
t udp "10.9.8.7" "docker run -d --name dns -p 15354:53/udp alpine sh -c 'apk add -q dnsmasq >/dev/null 2>&1; dnsmasq -k --address=/kiri.test/10.9.8.7' >/dev/null; for i in \$(seq 40); do out=\$(nslookup -port=15354 kiri.test 127.0.0.1 2>/dev/null) && break; sleep 0.5; done; echo \"\$out\" | grep -o 10.9.8.7"
t ipv6-network "true" "docker network create --ipv6 --subnet fd00:4c:1::/64 lh6 >/dev/null && docker network inspect lh6 -f '{{.EnableIPv6}}'"
t ipv6-ping "1 packets received" "docker run -d --name v6a --network lh6 nginx:alpine >/dev/null; docker run --rm --network lh6 alpine ping -6 -c 1 -W 2 v6a"
t ipv6-wget "Welcome to nginx" "docker run --rm --network lh6 alpine wget -qO- -T 5 'http://[fd00:4c:1::2]'"
t ipv6-host-nat "masquerade" "apk add -q nftables >/dev/null 2>&1; nft list table ip6 kiri"
t ipv6-port "Welcome to nginx" "docker run -d --name web6 -p 18082:80 nginx:alpine >/dev/null; for i in \$(seq 30); do curl -sg 'http://[::1]:18082' && break; sleep 0.3; done"
t ipv6-cleanup "gone" "docker rm -f v6a web6 >/dev/null; docker network rm lh6 >/dev/null; for i in \$(seq 20); do nft list table ip6 kiri >/dev/null 2>&1 || { echo gone; break; }; sleep 0.1; done"
t ipv6-forwarding-restored "0" "cat /proc/sys/net/ipv6/conf/all/forwarding"
# A build's --add-host host-gateway is the bridge gateway, as for docker run (not the Mac VM's 198.19.248.254).
t build-host-gateway "GW-OK" "gw=\$(docker network inspect bridge --format '{{(index .IPAM.Config 0).Gateway}}'); mkdir -p /bhg && printf 'FROM alpine:3.21\\nRUN grep -w hgw /etc/hosts\\n' > /bhg/Dockerfile && DOCKER_BUILDKIT=0 docker build --no-cache --add-host hgw:host-gateway /bhg 2>&1 | grep -q \"\$gw\" && echo GW-OK"
t cleanup "web" "docker rm -f web db dns"
$D rm -f kiri-linux-host >/dev/null
echo "$PASS passed, $FAIL failed"
[ "$FAIL" = 0 ]
