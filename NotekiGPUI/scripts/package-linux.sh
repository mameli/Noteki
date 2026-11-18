#!/bin/sh
# Builds Noteki for Linux into target/release/noteki-linux-<arch>.tar.gz:
#   bin/noteki, share/applications/app.noteki.noteki.desktop and the icon,
# laid out as a prefix (~/.local or /usr/local), so unpacking it there
# installs it:
#   tar -xzf noteki-linux-x86_64.tar.gz -C ~/.local
# Needs the libraries GPUI links (Debian/Ubuntu):
#   sudo apt install libxkbcommon-x11-dev libwayland-dev libfontconfig-dev \
#     libfreetype-dev libx11-xcb-dev libvulkan1
set -eu
cd "$(dirname "$0")/.."

cargo build --release
arch=$(uname -m)
root="target/release/noteki-linux"
rm -rf "$root"
mkdir -p "$root/bin" "$root/share/applications" "$root/share/icons/hicolor/256x256/apps" \
	"$root/share/icons/hicolor/512x512/apps"
cp target/release/noteki-gpui "$root/bin/noteki"
cp linux/app.noteki.noteki.desktop "$root/share/applications/"
icons="../Noteki/Assets.xcassets/AppIcon.appiconset"
cp "$icons/icon_256x256.png" "$root/share/icons/hicolor/256x256/apps/app.noteki.noteki.png"
cp "$icons/icon_512x512.png" "$root/share/icons/hicolor/512x512/apps/app.noteki.noteki.png"
tar -czf "target/release/noteki-linux-$arch.tar.gz" -C "$root" .
echo "target/release/noteki-linux-$arch.tar.gz ($(du -sh "target/release/noteki-linux-$arch.tar.gz" | cut -f1))"
