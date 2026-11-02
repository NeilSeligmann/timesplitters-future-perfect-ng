#!/bin/sh
# Wrap the launcher binary in an AppImage (T4336). Usage: appimage.sh <binary> <arch: x86_64|aarch64> <out.AppImage>
# appimagetool is pinned by version and SHA-256; it runs extracted so no FUSE is needed.
set -eu
bin=$1 arch=$2 out=$3
tool_version=1.9.1
case "$arch" in
  x86_64) tool_sha=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0 ;;
  aarch64) tool_sha=f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158 ;;
  *) echo "unknown arch $arch" >&2; exit 2 ;;
esac
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tool="$work/appimagetool"
curl -fsSL -o "$tool" "https://github.com/AppImage/appimagetool/releases/download/$tool_version/appimagetool-$arch.AppImage"
echo "$tool_sha  $tool" | sha256sum -c - >/dev/null
chmod +x "$tool"
app="$work/AppDir"
mkdir -p "$app/usr/bin"
cp "$bin" "$app/usr/bin/tsfp-launcher"
chmod +x "$app/usr/bin/tsfp-launcher"
ln -s usr/bin/tsfp-launcher "$app/AppRun"
cat > "$app/tsfp-launcher.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=TimeSplitters: Future Perfect NG
Exec=tsfp-launcher
Icon=tsfp-launcher
Categories=Game;
DESKTOP
# Placeholder icon until branding (T4336): a 1x1 PNG keeps appimagetool happy.
printf '\211PNG\r\n\032\n\000\000\000\rIHDR\000\000\000\001\000\000\000\001\010\006\000\000\000\037\025\304\211\000\000\000\rIDATx\234c\370\017\000\000\001\001\000\005\030\330N\000\000\000\000IEND\256B`\202' > "$app/tsfp-launcher.png"
ARCH=$arch APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream "$app" "$out" >/dev/null
echo "built $out"
