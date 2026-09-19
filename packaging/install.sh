#!/usr/bin/env bash
# Install 8BitDo R8 (daemon + GUI + autostart)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BIN="$PREFIX/bin"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
CFG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/r8ctl"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

echo "==> Building release…"
cargo build --release --manifest-path "$ROOT/Cargo.toml"

echo "==> Installing binaries to $BIN"
mkdir -p "$BIN" "$CFG_DIR" "$UNIT_DIR" "$APP_DIR"
install -m 755 "$ROOT/target/release/r8d" "$BIN/r8d"
install -m 755 "$ROOT/target/release/r8-gui" "$BIN/r8-gui"

if [[ ! -f "$CFG_DIR/config.json" ]]; then
  cat > "$CFG_DIR/config.json" <<'EOF'
{
  "buttons": {
    "side": "default",
    "extra": "default"
  },
  "software_remap": true,
  "saved_to_mouse": false,
  "language": "en"
}
EOF
fi

echo "==> systemd user service"
cat > "$UNIT_DIR/r8d.service" <<EOF
[Unit]
Description=8BitDo R8 remapper daemon
After=default.target

[Service]
Type=simple
ExecStart=$BIN/r8d --config $CFG_DIR/config.json
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
EOF

systemctl --user daemon-reload
systemctl --user enable --now r8d.service

echo "==> .desktop launcher"
cat > "$APP_DIR/r8-gui.desktop" <<EOF
[Desktop Entry]
Name=8BitDo R8 Settings
Comment=Configure Retro R8 mouse on Linux
Exec=$BIN/r8-gui
Icon=input-mouse
Terminal=false
Type=Application
Categories=Settings;HardwareSettings;
EOF

echo ""
echo "Done."
echo "  GUI:     r8-gui"
echo "  Daemon:  systemctl --user status r8d"
echo "  Config:  $CFG_DIR/config.json"
echo ""
echo "To save maps in the mouse (and uninstall software later):"
echo "  open r8-gui → pick mouse actions → Save in mouse settings"
echo ""
echo "Uninstall later with: $ROOT/packaging/uninstall.sh"
