#!/bin/bash
# Engine-only latency: one container lifecycle (create, start, wait, delete)
# and one exec, straight over the Docker API with curl, no docker CLI.
#   SOCK=/run/dockerd-bench.sock bench/api.sh docker-engine
set -u
label=$1; S=${SOCK:?}; OUT=${OUT:-$(dirname "$0")/results}; RUNS=${RUNS:-30}
c() { curl -s --unix-socket "$S" "$@"; }
c -X POST -H 'Content-Type: application/json' -d '{"Image":"alpine","Cmd":["true"]}' http://x/containers/create >/dev/null
cat > /tmp/api-run.sh <<R
c() { curl -s --unix-socket "$S" "\$@"; }
id=\$(c -X POST -H 'Content-Type: application/json' -d '{"Image":"alpine","Cmd":["true"]}' http://x/containers/create | jq -r .Id)
c -X POST http://x/containers/\$id/start >/dev/null; c -X POST http://x/containers/\$id/wait >/dev/null; c -X DELETE http://x/containers/\$id >/dev/null
R
id=$(c -X POST -H 'Content-Type: application/json' -d '{"Image":"alpine","Cmd":["sleep","600"]}' http://x/containers/create | jq -r .Id); c -X POST "http://x/containers/$id/start" >/dev/null
cat > /tmp/api-exec.sh <<R
c() { curl -s --unix-socket "$S" "\$@"; }
e=\$(c -X POST -H 'Content-Type: application/json' -d '{"Cmd":["true"],"AttachStdout":true}' http://x/containers/$id/exec | jq -r .Id)
c -X POST -H 'Content-Type: application/json' -d '{}' http://x/exec/\$e/start >/dev/null
R
hyperfine --warmup 3 --runs "$RUNS" --export-json "$OUT/$label.api-run.json" 'bash /tmp/api-run.sh' >/dev/null 2>&1
hyperfine --ignore-failure --warmup 3 --runs "$RUNS" --export-json "$OUT/$label.api-exec.json" 'bash /tmp/api-exec.sh' >/dev/null 2>&1
c -X DELETE "http://x/containers/$id?force=true" >/dev/null
printf '%-14s API run %4s ms (min %4s)   API exec %4s ms (min %4s)\n' "$label" $(jq -r '.results[0] | "\(.median*1000|round) \(.min*1000|round)"' "$OUT/$label.api-run.json") $(jq -r '.results[0] | "\(.median*1000|round) \(.min*1000|round)"' "$OUT/$label.api-exec.json")
