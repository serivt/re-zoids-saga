#!/usr/bin/env bash
# Builds the macOS package: a universal (Apple Silicon and Intel) app in a
# disk image, dist/re-zoids-saga-<version>-macos.dmg.
# Usage: tools/package/macos.sh [version]

source "$(dirname "$0")/common.sh"

export MACOSX_DEPLOYMENT_TARGET=11.0
TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
for target in "${TARGETS[@]}"; do
    cargo build --release --locked -p launcher --features packaged --target "$target"
done

APP="$STAGE/$BUNDLE_NAME.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/$EXECUTABLE" \
    "target/${TARGETS[0]}/release/launcher" \
    "target/${TARGETS[1]}/release/launcher"
cp assets/icons/re-zoids-saga.icns "$APP/Contents/Resources/$EXECUTABLE.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$APP_NAME</string>
    <key>CFBundleDisplayName</key><string>$APP_NAME</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleExecutable</key><string>$EXECUTABLE</string>
    <key>CFBundleIconFile</key><string>$EXECUTABLE</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${VERSION#v}</string>
    <key>CFBundleVersion</key><string>${VERSION#v}</string>
    <key>LSMinimumSystemVersion</key><string>$MACOSX_DEPLOYMENT_TARGET</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.role-playing-games</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>GPL-3.0-only</string>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP"
add_texts "$STAGE"
ln -s /Applications "$STAGE/Applications"

DMG="$DIST/$EXECUTABLE-$VERSION-macos.dmg"
rm -f "$DMG"
hdiutil create -volname "$BUNDLE_NAME $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
echo "$DMG"
