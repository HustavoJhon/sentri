#!/usr/bin/env bash
# Runs the same quality gate used before every commit and in CI.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --quiet
