#!/bin/sh
# Builds `target/release/Noteki GPUI.app`: the release binary, the Swift app's
# icon and the document types Takku looks for (SPEC › Takku): the Editor role
# for Markdown, the Viewer role for folders.
#   scripts/bundle.sh            build the app
#   scripts/bundle.sh --open F   build it, then open F with it
set -eu
cd "$(dirname "$0")/.."

cargo build --release --offline
app="target/release/Noteki GPUI.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/noteki-gpui "$app/Contents/MacOS/noteki-gpui"

icons="../Noteki/Assets.xcassets/AppIcon.appiconset"
iconset="$(mktemp -d)/Noteki.iconset"
mkdir -p "$iconset"
cp "$icons"/icon_*.png "$iconset/"
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Noteki.icns"
rm -rf "$(dirname "$iconset")"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key><string>Noteki GPUI</string>
	<key>CFBundleDisplayName</key><string>Noteki GPUI</string>
	<key>CFBundleIdentifier</key><string>app.noteki.noteki-gpui</string>
	<key>CFBundleExecutable</key><string>noteki-gpui</string>
	<key>CFBundleIconFile</key><string>Noteki</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleShortVersionString</key><string>$version</string>
	<key>CFBundleVersion</key><string>$version</string>
	<key>LSMinimumSystemVersion</key><string>13.0</string>
	<key>NSHighResolutionCapable</key><true/>
	<key>NSSupportsAutomaticGraphicsSwitching</key><true/>
	<key>CFBundleDocumentTypes</key>
	<array>
		<dict>
			<key>CFBundleTypeName</key><string>Markdown</string>
			<key>CFBundleTypeRole</key><string>Editor</string>
			<key>LSHandlerRank</key><string>Alternate</string>
			<key>LSItemContentTypes</key><array><string>net.daringfireball.markdown</string></array>
		</dict>
		<dict>
			<key>CFBundleTypeName</key><string>Folder</string>
			<key>CFBundleTypeRole</key><string>Viewer</string>
			<key>LSHandlerRank</key><string>Alternate</string>
			<key>LSItemContentTypes</key><array><string>public.folder</string></array>
		</dict>
	</array>
</dict>
</plist>
PLIST

# Ad-hoc signature, so Gatekeeper and Launch Services take the bundle as is.
codesign --force --sign - "$app" >/dev/null 2>&1 || true
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app" || true
echo "$app ($(du -sh "$app" | cut -f1))"

if [ "${1:-}" = "--open" ] && [ -n "${2:-}" ]; then
	open -a "$PWD/$app" "$2"
fi
