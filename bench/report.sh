#!/bin/bash
# Turns bench/results/*.json (git-ignored; finished runs are kept outside this repository) into a markdown table (medians; MB/s where it is a transfer).
OUT=${OUT:-$(dirname "$0")/results}
echo '| Engine | `run` ms | `exec` ms | stdout MB/s | stdin MB/s | bind write MB/s | bind read MB/s | net Gbit/s | build ms | pull ms |'
echo '|---|---|---|---|---|---|---|---|---|---|'
for n in "$OUT"/*.net.json; do
  l=$(jq -r .label "$n"); J="$OUT/$l"
  med() { [ -f "$J.$1.json" ] && jq -r ".results[0].median * 1000 | round" "$J.$1.json" || echo "n/a"; }
  mbs() { [ -f "$J.$1.json" ] && jq -r ".results[0].median as \$s | ($2 / \$s) | round" "$J.$1.json" || echo "n/a"; }
  echo "| $l | $(med run) | $(med exec) | $(mbs stdout 200) | $(mbs stdin 200) | $(mbs bind-write 1024) | $(mbs bind-read 1024) | $(jq -r .net_gbps "$n") | $(med build) | $(med pull) |"
done
