#!/usr/bin/env bash
# Starts the visual test-user MCP server (see docs/testing/visual-test-user.md).
# Builds the app and kopercad-cli first when they are missing. Extra arguments
# go to `kopercad-cli test-user` (--size 1920x1080, --project FILE, --connect PORT).
set -euo pipefail
cd "$(dirname "$0")/.."
bin=${KOPERCAD_BIN_DIR:-target/debug}
if [ ! -x "$bin/kopercad" ] || [ ! -x "$bin/kopercad-cli" ]; then
  cargo build -p novo-app -p novo-cli >&2
fi
exec "$bin/kopercad-cli" test-user "$@"
