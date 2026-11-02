#!/bin/sh
# Regenerates NOTICE from Cargo.lock and the pinned non-Rust components.
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
tmp=$(mktemp "${TMPDIR:-/tmp}/kiri-notice.XXXXXX")
trap 'rm -f "$tmp"' EXIT
cargo about generate --all-features --workspace --locked about.hbs -o "$tmp"
notice() {
  title=$1 url=$2
  printf '\n%s\n%s\nSource: %s\n\n' "$title" '----------------------------------------------------------------------' "$url" >> "$tmp"
  curl -fsSL --retry 3 "$url" >> "$tmp"
  printf '\n' >> "$tmp"
}
notice 'Docker CLI 29.9.2 — Apache-2.0' 'https://raw.githubusercontent.com/docker/cli/v29.9.2/LICENSE'
notice 'Docker CLI 29.9.2 — upstream NOTICE' 'https://raw.githubusercontent.com/docker/cli/v29.9.2/NOTICE'
notice 'Docker Compose 5.6.1 — Apache-2.0' 'https://raw.githubusercontent.com/docker/compose/v5.6.1/LICENSE'
notice 'Docker Compose 5.6.1 — upstream NOTICE' 'https://raw.githubusercontent.com/docker/compose/v5.6.1/NOTICE'
notice 'Docker Buildx 0.38.1 — Apache-2.0' 'https://raw.githubusercontent.com/docker/buildx/v0.38.1/LICENSE'
notice 'Grype 0.120.1 — Apache-2.0' 'https://raw.githubusercontent.com/anchore/grype/v0.120.1/LICENSE'
notice 'Phosphor Icons 2.1.1 — MIT' 'https://unpkg.com/@phosphor-icons/core@2.1.1/LICENSE'
notice 'Simple Icons 16.35.0 — CC0' 'https://unpkg.com/simple-icons@16.35.0/LICENSE.md'
notice 'Linux guest kernel — GPL-2.0-only' 'https://www.gnu.org/licenses/old-licenses/gpl-2.0.txt'
printf '\nGuest kernel source offer\n----------------------------------------------------------------------\n\n' >> "$tmp"
cat guest/kernel/SOURCE_OFFER.md >> "$tmp"
sed -e 's/\r$//' -e 's/[[:blank:]]*$//' "$tmp" > "$tmp.clean"
mv "$tmp.clean" "$tmp"
mv "$tmp" NOTICE
