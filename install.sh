#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_PNG_DIR="$HOME/.local/share/icons/hicolor/512x512/apps"
ICON_SVG_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"

echo "⚙️ Building ohhPlayer in release mode..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"

TARGET_DIR=$(cargo metadata --format-version 1 --manifest-path "$SCRIPT_DIR/Cargo.toml" 2>/dev/null | grep -o '"target_directory":"[^"]*' | cut -d'"' -f4)
BIN_PATH="$TARGET_DIR/release/ohhplayer"

echo "📂 Installing binary and desktop icons..."
mkdir -p "$BIN_DIR" "$DESKTOP_DIR" "$ICON_PNG_DIR" "$ICON_SVG_DIR"

cp "$BIN_PATH" "$BIN_DIR/ohhplayer"
chmod +x "$BIN_DIR/ohhplayer"

cp "$SCRIPT_DIR/assets/ohhplayer.png" "$ICON_PNG_DIR/ohhplayer.png"
cp "$SCRIPT_DIR/assets/ohhplayer.svg" "$ICON_SVG_DIR/ohhplayer.svg"
cp "$SCRIPT_DIR/assets/ohhplayer.desktop" "$DESKTOP_DIR/ohhplayer.desktop"

# Refresh desktop database if tool is present
if command -v update-desktop-database &> /dev/null; then
    update-desktop-database "$DESKTOP_DIR" || true
fi

echo "✅ ohhPlayer installed successfully!"
echo "   Binary location: $BIN_DIR/ohhplayer"
echo "   PNG Icon:        $ICON_PNG_DIR/ohhplayer.png"
echo "   SVG Icon:        $ICON_SVG_DIR/ohhplayer.svg"
echo "   Desktop Entry:   $DESKTOP_DIR/ohhplayer.desktop"
echo ""
echo "🎉 You can now open ohhPlayer from your Application Launcher or terminal!"
