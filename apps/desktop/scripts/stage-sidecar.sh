#!/usr/bin/env bash
# Build the sentynyx-ner sidecar and stage it into src-tauri/binaries/ with
# the host target-triple suffix that Tauri's externalBin system expects.
# Run before `pnpm tauri dev` / `pnpm tauri build` / `cargo test --lib` the
# first time, or after pulling changes to the sidecar source.
#
#   ./scripts/stage-sidecar.sh            # debug build
#   ./scripts/stage-sidecar.sh --release  # release build

set -euo pipefail

cd "$(dirname "$0")/../src-tauri"

PROFILE="dev"
PROFILE_DIR="debug"
if [[ "${1:-}" == "--release" ]]; then
  PROFILE="release"
  PROFILE_DIR="release"
fi

TRIPLE="$(rustc -vV | sed -n 's|host: ||p')"
echo "Building sentynyx-ner for $TRIPLE ($PROFILE)"

if [[ "$PROFILE" == "release" ]]; then
  cargo build --release --bin sentynyx-ner
else
  cargo build --bin sentynyx-ner
fi

mkdir -p binaries
cp "target/$PROFILE_DIR/sentynyx-ner" "binaries/sentynyx-ner-$TRIPLE"
echo "Staged: binaries/sentynyx-ner-$TRIPLE"
