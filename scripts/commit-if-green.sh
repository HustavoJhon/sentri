#!/usr/bin/env bash
# Usage: scripts/commit-if-green.sh "<conventional commit message>" [--amend]
# Stages everything and commits only when the full quality gate passes.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all
log="$(mktemp)"
trap 'rm -f "$log"' EXIT
if ! ./scripts/check.sh >"$log" 2>&1; then
    grep -E "^(error|warning)|-->|panicked|FAILED" "$log" | head -40
    echo "quality gate failed; nothing committed" >&2
    exit 1
fi
git add -A
git commit -q "${@:2}" -m "$1"
git log --oneline -1
