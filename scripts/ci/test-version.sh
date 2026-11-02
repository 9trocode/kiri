#!/bin/sh
# A build that isn't a release takes the released version (the update feed's,
# not the newest tag: stray v0.2.x tags predate 0.1.x), then -dev. and the CI
# run's number (0.1.8-dev.42), so it says which build it is and never offers
# itself an "update" (ui/mod.rs). Releases take theirs from the tag instead.
#   scripts/ci/test-version.sh
set -eu
last=$(curl -fsS -A kiri-ci https://download.runkiri.com/latest.json | sed -n 's/.*"version": *"\([0-9]*\.[0-9]*\.[0-9]*\)".*/\1/p' | head -1 || true)
v="${last:-0.0.0}-dev.${GITHUB_RUN_NUMBER:-0}"
perl -pi -e 's/^version = "[^"]*"/version = "'"$v"'"/ && ($done = 1) unless $done' Cargo.toml
grep -m1 '^version = ' Cargo.toml
