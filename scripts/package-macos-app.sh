#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-release}"
TARGET_DIR="$ROOT_DIR/target/$PROFILE"
APP_BUNDLE="$ROOT_DIR/dist/QuixiDesk.app"
CONTENTS="$APP_BUNDLE/Contents"

echo "[0/4] Building Rust workspace ($PROFILE)..."
if [ "$PROFILE" = "release" ]; then
    cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --release --workspace
else
    cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --workspace
fi

echo "[1/4] Creating QuixiDesk.app bundle structure at $APP_BUNDLE..."
rm -rf "$APP_BUNDLE"
mkdir -p "$CONTENTS/MacOS"
mkdir -p "$CONTENTS/Helpers"
mkdir -p "$CONTENTS/Resources"

echo "[2/4] Writing Contents/Info.plist..."
cat > "$CONTENTS/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>QuixiDesk</string>
    <key>CFBundleExecutable</key>
    <string>QuixiDesk</string>
    <key>CFBundleIconFile</key>
    <string>quixi.icns</string>
    <key>CFBundleIdentifier</key>
    <string>com.quixi.desktop</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>QuixiDesk</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>3.6556.2</string>
    <key>CFBundleVersion</key>
    <string>3.6556.2</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>NSAppTransportSecurity</key>
    <dict>
        <key>NSAllowsArbitraryLoads</key>
        <true/>
    </dict>
</dict>
</plist>
EOF

echo "[3/4] Copying compiled Rust binaries into Contents/MacOS and Contents/Helpers..."
cp "$TARGET_DIR/QuixiDesk" "$CONTENTS/MacOS/QuixiDesk"
chmod +x "$CONTENTS/MacOS/QuixiDesk"

for helper in quixi-disclaimer quixi-chrome-native-host quixi-ssh-askpass quixi-guest-daemon quixi-mcp-builtin; do
    if [ -f "$TARGET_DIR/$helper" ]; then
        cp "$TARGET_DIR/$helper" "$CONTENTS/Helpers/$helper"
        chmod +x "$CONTENTS/Helpers/$helper"
    fi
done
mkdir -p "$HOME/.local/bin"
if [ -f "$TARGET_DIR/quixi-mcp-builtin" ]; then
    rm -f "$HOME/.local/bin/quixi-mcp-builtin"
    cp "$TARGET_DIR/quixi-mcp-builtin" "$HOME/.local/bin/quixi-mcp-builtin"
    chmod +x "$HOME/.local/bin/quixi-mcp-builtin"
    codesign -s - -f "$HOME/.local/bin/quixi-mcp-builtin" 2>/dev/null || true
fi

# Create compatibility symlinks matching Claude.app helper names
ln -sf quixi-disclaimer "$CONTENTS/Helpers/disclaimer"
ln -sf quixi-chrome-native-host "$CONTENTS/Helpers/chrome-native-host"
ln -sf quixi-ssh-askpass "$CONTENTS/Helpers/claude-ssh-askpass"
ln -sf quixi-guest-daemon "$CONTENTS/Helpers/sdk-daemon"

echo "[4/4] Linking/copying Resources into Contents/Resources..."
cp "$ROOT_DIR/resources/quixi.icns" "$CONTENTS/Resources/quixi.icns" 2>/dev/null || true
cp "$ROOT_DIR/resources/quixi-logo.svg" "$CONTENTS/Resources/quixi-logo.svg" 2>/dev/null || true
cp "$ROOT_DIR/resources/quixi-mark.svg" "$CONTENTS/Resources/quixi-mark.svg" 2>/dev/null || true
cp "$ROOT_DIR/resources/en-US.json" "$CONTENTS/Resources/en-US.json"
ln -sf "$ROOT_DIR/resources/ion-dist" "$CONTENTS/Resources/ion-dist"
ln -sf "$ROOT_DIR/resources/renderer" "$CONTENTS/Resources/renderer"
ln -sf "$ROOT_DIR/resources/preload" "$CONTENTS/Resources/preload"
ln -sf "$ROOT_DIR/resources/bootstrap" "$CONTENTS/Resources/bootstrap"
ln -sf "$ROOT_DIR/resources/fonts" "$CONTENTS/Resources/fonts"
ln -sf "$ROOT_DIR/resources/bundled-skills" "$CONTENTS/Resources/bundled-skills"
ln -sf "$ROOT_DIR/resources/bundled-skills-extracted" "$CONTENTS/Resources/bundled-skills-extracted"
ln -sf "$ROOT_DIR/resources/artifact-libs" "$CONTENTS/Resources/artifact-libs"

echo "Done! Built $APP_BUNDLE"
