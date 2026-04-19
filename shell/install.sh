#!/usr/bin/env bash
# Coati shell plugin installer
# Usage: ./shell/install.sh [--shell zsh|bash|fish|auto]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

TARGET_SHELL="auto"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --shell) TARGET_SHELL="$2"; shift 2 ;;
        -h|--help)
            cat <<EOF
Coati shell plugin installer

Usage: $0 [--shell zsh|bash|fish|auto]

With --shell auto (default), detects from \$SHELL.
Appends a single source line to your rc file. Idempotent.
EOF
            exit 0 ;;
        *) echo "unknown flag: $1" >&2; exit 1 ;;
    esac
done

if [[ "$TARGET_SHELL" == "auto" ]]; then
    case "$(basename "${SHELL:-/bin/bash}")" in
        zsh)  TARGET_SHELL="zsh" ;;
        bash) TARGET_SHELL="bash" ;;
        fish) TARGET_SHELL="fish" ;;
        *) echo "Could not auto-detect shell; use --shell explicitly" >&2; exit 1 ;;
    esac
fi

MARKER="# coati shell plugin"
case "$TARGET_SHELL" in
    zsh)
        PLUGIN="$REPO_ROOT/shell/zsh/coati.plugin.zsh"
        RC="$HOME/.zshrc" ;;
    bash)
        PLUGIN="$REPO_ROOT/shell/bash/coati.bash"
        RC="$HOME/.bashrc" ;;
    fish)
        PLUGIN="$REPO_ROOT/shell/fish/coati.fish"
        RC="$HOME/.config/fish/config.fish"
        mkdir -p "$(dirname "$RC")" ;;
    *) echo "unsupported shell: $TARGET_SHELL" >&2; exit 1 ;;
esac

if [[ ! -f "$PLUGIN" ]]; then
    echo "plugin not found: $PLUGIN" >&2
    echo "(If you picked fish, make sure Task 11 / fish plugin has been implemented.)" >&2
    exit 1
fi

if [[ -f "$RC" ]] && grep -qF "$MARKER" "$RC"; then
    echo "✓ coati plugin already installed in $RC"
    exit 0
fi

{
    echo ""
    echo "$MARKER"
    echo "source \"$PLUGIN\""
} >> "$RC"

echo "✓ appended coati source line to $RC"
echo "  run:  . \"$RC\"   (or restart your terminal)"

# Desktop app is optional.
if command -v coati-desktop >/dev/null 2>&1; then
  echo "Desktop app detected at $(command -v coati-desktop)."
else
  echo ""
  echo "Optional: install the desktop app with:"
  echo "  cargo build -p coati-desktop --release --features desktop && ./scripts/install-desktop.sh"
fi

echo ""
echo "Tip: install the desktop app + voice PTT with:"
echo "  ./scripts/install-desktop.sh --with-voice"
