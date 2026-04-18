#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BIN_SRC="$REPO_ROOT/target/release/coati-desktop"
BIN_DST="$HOME/.local/bin/coati-desktop"

if [ ! -x "$BIN_SRC" ]; then
  echo "error: $BIN_SRC not found." >&2
  echo "Run: cargo build -p coati-desktop --release --features desktop" >&2
  exit 1
fi

mkdir -p "$(dirname "$BIN_DST")"
install -m 0755 "$BIN_SRC" "$BIN_DST"

APPS_DIR="$HOME/.local/share/applications"
mkdir -p "$APPS_DIR"
cat > "$APPS_DIR/coati-desktop.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Coati
Comment=Your Linux copilot
Exec=$BIN_DST
Icon=coati
Categories=Utility;
Terminal=false
StartupNotify=true
EOF

echo "Installed coati-desktop to $BIN_DST"
echo "Launch with: coati-desktop"
