#!/bin/bash
# Times the hot paths of whatever engine `docker` talks to: container start,
# exec, and 200 MB each way. Run it against Kiri and against another engine
# (Docker Desktop's WSL integration, dockerd) on the same machine, back to
# back, and compare the lines. Medians of 30 for the latencies.
#
#   DOCKER_HOST=unix:///var/lib/kiri-daemon/.kiri/run/docker.sock bench/compare.sh kiri
#   DOCKER_HOST=unix:///var/run/docker.sock bench/compare.sh desktop
#
# Note: the "in" figure only means something if every byte arrived; the
# script prints the count the container saw next to it.
set -u
label=${1:-${DOCKER_HOST:-default}}
docker pull -q alpine >/dev/null 2>&1
for i in 1 2 3; do docker run --rm alpine true >/dev/null 2>&1; done   # warm
now() { date +%s%N; }
med() { sort -n | awk '{a[NR]=$1} END {print a[int((NR+1)/2)]}'; }
run=$(mktemp); ex=$(mktemp)
for i in $(seq 30); do t=$(now); docker run --rm alpine true >/dev/null 2>&1; echo $(( ($(now)-t)/1000000 )); done > "$run"
name=bench-$$
docker run -d --name "$name" alpine sleep 600 >/dev/null
for i in $(seq 30); do t=$(now); docker exec "$name" true >/dev/null 2>&1; echo $(( ($(now)-t)/1000000 )); done > "$ex"
t=$(now); docker run --rm alpine sh -c 'head -c 200000000 /dev/zero' > /dev/null 2>&1; out=$(( ($(now)-t)/1000000 ))
t=$(now); seen=$(head -c 200000000 /dev/zero | docker run --rm -i alpine wc -c 2>/dev/null | tr -d ' \r'); in=$(( ($(now)-t)/1000000 ))
docker rm -f "$name" >/dev/null 2>&1
printf '%-10s run %3s ms (min %3s)  exec %3s ms (min %3s)  200MB out %5s ms = %4d MB/s  200MB in %5s ms = %4d MB/s (container saw %s bytes)\n' \
  "$label" "$(med <"$run")" "$(sort -n "$run" | head -1)" "$(med <"$ex")" "$(sort -n "$ex" | head -1)" \
  "$out" $(( 200000 / (out>0?out:1) )) "$in" $(( 200000 / (in>0?in:1) )) "${seen:-?}"
rm -f "$run" "$ex"
