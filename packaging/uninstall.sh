#!/usr/bin/env bash
# Remove installed binaries and user autostart (keeps config + mouse firmware maps)
set -euo pipefail
PREFIX="${PREFIX:-$HOME/.local}"
BIN="$PREFIX/bin"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

systemctl --user disable --now r8d.service 2>/dev/null || true
rm -f "$UNIT_DIR/r8d.service"
rm -f "$BIN/r8d" "$BIN/r8-gui"
rm -f "$APP_DIR/r8-gui.desktop"
systemctl --user daemon-reload 2>/dev/null || true

echo "Binaries and autostart removed."
echo "Config in ~/.config/r8ctl was kept (delete manually if you want)."
echo "Firmware maps in the mouse are UNCHANGED — they still remember your buttons."
