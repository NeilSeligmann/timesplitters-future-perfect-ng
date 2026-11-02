#!/bin/sh
# Wrap the launcher binary in an unsigned .app inside a .dmg (T4336). Usage: macos_dmg.sh <binary> <version> <out.dmg>
# Code signing and notarisation need an Apple Developer ID (placeholder); until then users right-click > Open once.
set -eu
bin=$1 version=$2 out=$3
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
app="$work/TimeSplitters NG Launcher.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/tsfp-launcher"
chmod +x "$app/Contents/MacOS/tsfp-launcher"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>TimeSplitters NG Launcher</string>
  <key>CFBundleIdentifier</key><string>io.github.neilseligmann.tsfp-ng.launcher</string>
  <key>CFBundleExecutable</key><string>tsfp-launcher</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
ln -s /Applications "$work/Applications"
hdiutil create -quiet -volname "TimeSplitters NG Launcher" -srcfolder "$work" -ov -format UDZO "$out"
echo "built $out"
