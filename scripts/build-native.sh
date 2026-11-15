#!/usr/bin/env bash
# Build the native mod through the Horizon launcher.
#
#   ./scripts/build-native.sh
#   HORIZON_LAUNCHER=/path/to/horizon-launcher ./scripts/build-native.sh
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bundle="$here/../bundle"
launcher="${HORIZON_LAUNCHER:-horizon-launcher}"

if [[ ! -f "$bundle/horizon-mixin.toml" ]]; then
  echo "error: $bundle/horizon-mixin.toml is missing" >&2
  exit 1
fi
if ! command -v "$launcher" >/dev/null 2>&1 && [[ ! -x "$launcher" ]]; then
  cat >&2 <<MSG
error: the Horizon launcher was not found.

Set HORIZON_LAUNCHER to the launcher executable:
  HORIZON_LAUNCHER="\$HOME/Applications/horizon-launcher" "$0"

The launcher must have Minecraft 26.3 prepared and the Nightly
nightly-2026-11-01-c9d474d installed (README section 4).
MSG
  exit 1
fi
echo "Building $bundle with $launcher ..."
exec "$launcher" native-build "$bundle" --trust-native-code
