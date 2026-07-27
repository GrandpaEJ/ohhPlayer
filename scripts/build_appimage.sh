#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "📦 Building release binary..."
cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml"

TARGET_DIR=$(cargo metadata --format-version 1 --manifest-path "$PROJECT_DIR/Cargo.toml" 2>/dev/null | grep -o '"target_directory":"[^"]*' | cut -d'"' -f4)
BIN_PATH="$TARGET_DIR/release/ohhplayer"

if [ ! -f "$BIN_PATH" ]; then
    echo "❌ Error: Could not find binary at $BIN_PATH"
    exit 1
fi

APP_DIR="$PROJECT_DIR/target/AppDir"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/usr/bin"
mkdir -p "$APP_DIR/usr/share/applications"
mkdir -p "$APP_DIR/usr/share/icons/hicolor/512x512/apps"
mkdir -p "$APP_DIR/usr/share/icons/hicolor/scalable/apps"

echo "📂 Populating AppDir..."
cp "$BIN_PATH" "$APP_DIR/usr/bin/ohhplayer"
cp "$PROJECT_DIR/assets/ohhplayer.desktop" "$APP_DIR/usr/share/applications/ohhplayer.desktop"
cp "$PROJECT_DIR/assets/ohhplayer.desktop" "$APP_DIR/ohhplayer.desktop"
cp "$PROJECT_DIR/assets/ohhplayer.svg" "$APP_DIR/usr/share/icons/hicolor/scalable/apps/ohhplayer.svg"
cp "$PROJECT_DIR/assets/ohhplayer.svg" "$APP_DIR/ohhplayer.svg"
cp "$PROJECT_DIR/assets/ohhplayer.png" "$APP_DIR/usr/share/icons/hicolor/512x512/apps/ohhplayer.png"
cp "$PROJECT_DIR/assets/ohhplayer.png" "$APP_DIR/ohhplayer.png"
cp "$PROJECT_DIR/assets/ohhplayer.png" "$APP_DIR/.DirIcon"

# AppRun launcher
cat << 'EOF' > "$APP_DIR/AppRun"
#!/bin/bash
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH}"
exec "${HERE}/usr/bin/ohhplayer" "$@"
EOF
chmod +x "$APP_DIR/AppRun"

# Check for appimagetool
APPIMAGETOOL=""
if command -v appimagetool &> /dev/null; then
    APPIMAGETOOL="appimagetool"
elif [ -f "/tmp/appimagetool" ]; then
    APPIMAGETOOL="/tmp/appimagetool"
else
    echo "⬇️ Downloading appimagetool..."
    curl -sL https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage -o /tmp/appimagetool
    chmod +x /tmp/appimagetool
    APPIMAGETOOL="/tmp/appimagetool"
fi

echo "🚀 Packaging AppImage..."
ARCH=x86_64 "$APPIMAGETOOL" "$APP_DIR" "$PROJECT_DIR/target/ohhPlayer-x86_64.AppImage"

echo "✅ AppImage created successfully: $PROJECT_DIR/target/ohhPlayer-x86_64.AppImage"
