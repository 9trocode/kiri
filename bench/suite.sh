#!/bin/bash
# Benchmarks one engine through the docker-compatible CLI it is reached with.
# Every timing comes from hyperfine; results land in $OUT/<label>.json plus
# one summary line.  Run the engines one at a time, back to back.
#
#   DOCKER_HOST=unix:///run/dockerd-bench.sock bench/suite.sh docker-engine
#   CLI=nerdctl bench/suite.sh nerdctl
#
# Env: CLI (default docker), RUNS (default 30), OUT (default bench/results).
set -u
label=$1; CLI=${CLI:-docker}; RUNS=${RUNS:-30}; OUT=${OUT:-$(dirname "$0")/results}
mkdir -p "$OUT"; J="$OUT/$label"; rm -f "$J".*.json
hf() { hyperfine -N --warmup 3 --runs "$RUNS" --export-json "$J.$1.json" "${@:2}" >/dev/null 2>&1 || echo "  $1: failed"; }
hfs() { hyperfine --warmup 1 --runs "${RUNS_IO:-5}" --export-json "$J.$1.json" "${@:2}" >/dev/null 2>&1 || echo "  $1: failed"; }
$CLI pull -q alpine >/dev/null 2>&1; $CLI pull -q networkstatic/iperf3 >/dev/null 2>&1
for i in 1 2 3; do $CLI run --rm alpine true >/dev/null 2>&1; done
name=bench-$$; $CLI run -d --name "$name" alpine sleep 3600 >/dev/null 2>&1
hf run  "$CLI run --rm alpine true"
hf exec "$CLI exec $name true"
hfs stdout "$CLI run --rm alpine head -c 200000000 /dev/zero | cat >/dev/null"
seen=$($CLI run --rm -i alpine wc -c < <(head -c 200000000 /dev/zero) 2>/dev/null | tr -d ' \r')
[ "$seen" = 200000000 ] && hfs stdin "head -c 200000000 /dev/zero | $CLI run --rm -i alpine wc -c" || echo "  stdin: container saw ${seen:-0} of 200000000 bytes; not timed"
vol=$(mktemp -d /tmp/bench-vol.XXXX); chmod 777 "$vol"
hfs bind-write "$CLI run --rm -v $vol:/d alpine dd if=/dev/zero of=/d/f bs=1M count=1024 conv=fsync"
hfs bind-read  "$CLI run --rm -v $vol:/d alpine dd if=/d/f of=/dev/null bs=1M"
rm -rf "$vol"
$CLI run -d --name "$name-iperf" -p 127.0.0.1:15201:5201 networkstatic/iperf3 -s >/dev/null 2>&1; sleep 2
net=$(iperf3 -c 127.0.0.1 -p 15201 -t 5 -J 2>/dev/null | jq -r '.end.sum_received.bits_per_second // 0 | . / 1e9 | . * 100 | round / 100')
$CLI rm -f "$name-iperf" >/dev/null 2>&1
ctx=$(mktemp -d); printf 'FROM alpine\nRUN echo one > /one\nRUN echo two > /two\nRUN echo three > /three\n' > "$ctx/Dockerfile"
hfs build "$CLI build --no-cache -q -t bench-build $ctx >/dev/null"
$CLI rmi -f bench-build >/dev/null 2>&1; rm -rf "$ctx"
hfs pull "$CLI rmi -f alpine >/dev/null 2>&1; $CLI pull -q alpine"
$CLI rm -f "$name" >/dev/null 2>&1
med() { [ -f "$J.$1.json" ] && jq -r ".results[0].median * 1000 | round" "$J.$1.json" || echo -; }
mbs() { [ -f "$J.$1.json" ] && jq -r ".results[0].median as \$s | ($2 / \$s) | round" "$J.$1.json" || echo -; }
printf '%-14s run %4s ms  exec %4s ms  stdout %4s MB/s  stdin %4s MB/s  bind-write %4s MB/s  bind-read %4s MB/s  net %s Gbit/s  build %5s ms  pull %5s ms\n' \
  "$label" "$(med run)" "$(med exec)" "$(mbs stdout 200)" "$(mbs stdin 200)" "$(mbs bind-write 1024)" "$(mbs bind-read 1024)" "${net:-0}" "$(med build)" "$(med pull)" | tee -a "$OUT/summary.txt"
echo "{\"label\":\"$label\",\"net_gbps\":${net:-0}}" > "$J.net.json"
