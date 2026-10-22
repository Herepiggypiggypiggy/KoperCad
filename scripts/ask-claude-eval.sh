#!/usr/bin/env bash
# One command: the Ask Claude co-engineer test. Without a key it runs the scripted model (free,
# checks the test setup and the tools). With ANTHROPIC_API_KEY set it uses the real model (costs
# money; the key is never printed or written). Extra options go to the tool, for example
#   scripts/ask-claude-eval.sh --only create-dol-starter
#   ANTHROPIC_API_KEY=sk-ant-... scripts/ask-claude-eval.sh --real --out audit/ask-claude
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-target}
exec cargo run -q -p novo-cli -- ask-claude-eval "$@"
