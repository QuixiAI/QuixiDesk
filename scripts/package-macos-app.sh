#!/usr/bin/env bash
#
# Builds the Rust workspace and packages a relocatable dist/QuixiDesk.app bundle.
#
#   ./scripts/package-macos-app.sh            # release build (default)
#   ./scripts/package-macos-app.sh debug      # debug build
#   QUIXI_APP_SYMLINK_RESOURCES=1 ./scripts/package-macos-app.sh
#                                             # keep resources symlinked (fast iteration)
#
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-release}"
TARGET_DIR="$ROOT_DIR/target/$PROFILE"
APP_BUNDLE="$ROOT_DIR/dist/QuixiDesk.app"
CONTENTS="$APP_BUNDLE/Contents"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -1)"
VERSION="${VERSION:-0.0.0}"

# rustup installs live in ~/.cargo/bin, which non-login shells often miss.
if ! command -v cargo >/dev/null 2>&1; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

echo "[0/5] Building Rust workspace ($PROFILE, version $VERSION)..."
if [ "$PROFILE" = "release" ]; then
    cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --release --workspace
else
    cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --workspace
fi

echo "[1/5] Creating QuixiDesk.app bundle structure at $APP_BUNDLE..."
rm -rf "$APP_BUNDLE"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Helpers" "$CONTENTS/Resources"

echo "[2/5] Writing Contents/Info.plist..."
cat > "$CONTENTS/Info.plist" <<EOF
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
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>NSAppleEventsUsageDescription</key>
    <string>QuixiDesk drives AppleScript-enabled apps for its built-in AppleScript tools.</string>
    <key>NSAppleMusicUsageDescription</key>
    <string>QuixiDesk may read Apple Music state when a tool asks for it.</string>
    <key>NSCalendarsUsageDescription</key>
    <string>QuixiDesk may read your calendars when a tool asks for it.</string>
    <key>NSContactsUsageDescription</key>
    <string>QuixiDesk may read contacts when a tool asks for it.</string>
    <key>NSMicrophoneUsageDescription</key>
    <string>QuixiDesk records audio when you use voice input.</string>
    <key>NSCameraUsageDescription</key>
    <string>QuixiDesk uses the camera when you attach a photo or use a video tool.</string>
    <key>NSScreenCaptureDescription</key>
    <string>QuixiDesk captures the screen for its Computer Use tools.</string>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>NSAppTransportSecurity</key>
    <dict>
        <key>NSAllowsArbitraryLoads</key>
        <true/>
    </dict>
</dict>
</plist>
EOF

echo "[3/5] Copying compiled Rust binaries into Contents/MacOS and Contents/Helpers..."
cp "$TARGET_DIR/QuixiDesk" "$CONTENTS/MacOS/QuixiDesk"
chmod +x "$CONTENTS/MacOS/QuixiDesk"

for helper in quixi-disclaimer quixi-chrome-native-host quixi-ssh-askpass quixi-guest-daemon quixi-mcp-builtin; do
    if [ -f "$TARGET_DIR/$helper" ]; then
        cp "$TARGET_DIR/$helper" "$CONTENTS/Helpers/$helper"
        chmod +x "$CONTENTS/Helpers/$helper"
    else
        echo "  ! missing helper binary: $helper (build it first)" >&2
    fi
done

# Expose the built-in MCP server on PATH for external agents/tools.
install -d "$HOME/.local/bin"
if [ -f "$TARGET_DIR/quixi-mcp-builtin" ]; then
    rm -f "$HOME/.local/bin/quixi-mcp-builtin"
    cp "$TARGET_DIR/quixi-mcp-builtin" "$HOME/.local/bin/quixi-mcp-builtin"
    chmod +x "$HOME/.local/bin/quixi-mcp-builtin"
    codesign -s - -f "$HOME/.local/bin/quixi-mcp-builtin" 2>/dev/null || true
fi

# Compatibility symlinks matching the Claude.app helper names.
ln -sf quixi-disclaimer "$CONTENTS/Helpers/disclaimer"
ln -sf quixi-chrome-native-host "$CONTENTS/Helpers/chrome-native-host"
ln -sf quixi-ssh-askpass "$CONTENTS/Helpers/claude-ssh-askpass"
ln -sf quixi-guest-daemon "$CONTENTS/Helpers/sdk-daemon"

echo "[4/5] Installing Resources into Contents/Resources..."
copy_res() { # copy_res <relative-path>
    local rel="$1"
    local src="$ROOT_DIR/resources/$rel"
    local dst="$CONTENTS/Resources/$rel"
    [ -e "$src" ] || { echo "  ! missing resource: resources/$rel" >&2; return 0; }
    if [ "${QUIXI_APP_SYMLINK_RESOURCES:-0}" = "1" ]; then
        ln -sfn "$src" "$dst"
    else
        rm -rf "$dst"
        cp -R "$src" "$dst"
    fi
}

for res in quixi.icns quixi-logo.svg quixi-mark.svg quixi-spinner.gif en-US.json \
           ion-dist renderer preload bootstrap fonts bundled-skills \
           bundled-skills-extracted artifact-libs; do
    copy_res "$res"
done

echo "[5/5] Ad-hoc signing $APP_BUNDLE..."
# Ad-hoc signing keeps WKWebView, TCC prompts and helper binaries happy locally.
codesign --force --deep --sign - "$APP_BUNDLE" >/dev/null 2>&1 \
    || echo "  ! ad-hoc codesign failed (the app still runs unsigned)" >&2

SIZE="$(du -sh "$APP_BUNDLE" | awk '{print $1}')"
echo "Done! $APP_BUNDLE ($SIZE) — launch with: open \"$APP_BUNDLE\""
