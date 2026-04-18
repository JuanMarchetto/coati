#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v bats >/dev/null 2>&1; then
    echo "FAIL: bats not installed. apt install bats (Ubuntu) or brew install bats-core" >&2
    exit 1
fi

bats "$SCRIPT_DIR/zsh.bats" "$SCRIPT_DIR/bash.bats"
