#!/bin/sh
# Builds `target/release/Noteki.app`: the release binary, the icon, Sparkle
# (updates, ADR 0011) and the document types Takku looks for (SPEC › Takku):
# the Editor role for Markdown, the Viewer role for folders. The same bundle
# id as the Swift app it replaces, so its preferences and updates carry over.
#   scripts/bundle.sh            build the app, signed ad hoc
#   scripts/bundle.sh --open F   build it, then open F with it
# NOTEKI_IDENTITY signs it with that certificate instead, with the hardened
# runtime and a secure timestamp, as notarization asks (scripts/release.sh).
set -eu
cd "$(dirname "$0")/.."

cargo build --release
app="target/release/Noteki.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Frameworks"
cp target/release/noteki "$app/Contents/MacOS/noteki"

icons="../Noteki/Assets.xcassets/AppIcon.appiconset"
iconset="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$iconset"
cp "$icons"/icon_*.png "$iconset/"
iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"
rm -rf "$(dirname "$iconset")"

# Sparkle, as released (its framework, and sign_update for the appcast),
# downloaded once and checked.
sparkle_version=2.10.1
sparkle_sha=1a856d2a9edb8e628de88121d7be04cf0e2167f8641650b78fca00b14244cf27
sparkle="target/sparkle-$sparkle_version"
if [ ! -d "$sparkle/Sparkle.framework" ]; then
	archive="target/Sparkle-$sparkle_version.tar.xz"
	curl -fsSL -o "$archive" "https://github.com/sparkle-project/Sparkle/releases/download/$sparkle_version/Sparkle-$sparkle_version.tar.xz"
	echo "$sparkle_sha  $archive" | shasum -a 256 -c - >/dev/null
	mkdir -p "$sparkle"
	tar -xJf "$archive" -C "$sparkle" Sparkle.framework bin
	rm "$archive"
fi
ditto "$sparkle/Sparkle.framework" "$app/Contents/Frameworks/Sparkle.framework"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
build=$(sed -n 's/^build = //p' Cargo.toml | head -1)
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key><string>Noteki</string>
	<key>CFBundleDisplayName</key><string>Noteki</string>
	<key>CFBundleIdentifier</key><string>app.noteki.noteki</string>
	<key>CFBundleExecutable</key><string>noteki</string>
	<key>CFBundleIconFile</key><string>AppIcon</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleShortVersionString</key><string>$version</string>
	<key>CFBundleVersion</key><string>$build</string>
	<key>LSMinimumSystemVersion</key><string>15.0</string>
	<key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
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
	<!-- Updates with Sparkle (ADR 0011): the appcast of the latest GitHub Release, and the
	     public half of the EdDSA key that signs the disk images (account "noteki"). -->
	<key>SUEnableAutomaticChecks</key><true/>
	<key>SUFeedURL</key><string>https://github.com/mameli/Noteki/releases/latest/download/appcast.xml</string>
	<key>SUPublicEDKey</key><string>yiDSZZuHxZyVOGgVD1WcUiTePxFrTHbyMjLr9iIW+CE=</string>
</dict>
</plist>
PLIST

# Inside out: Sparkle's helpers, its framework, then the app.
identity="${NOTEKI_IDENTITY:--}"
sign() {
	if [ "$identity" = "-" ]; then
		codesign --force --sign - "$@"
	else
		codesign --force --sign "$identity" --timestamp --options runtime "$@"
	fi
}
framework="$app/Contents/Frameworks/Sparkle.framework"
sign "$framework/Versions/B/XPCServices/Installer.xpc"
sign --preserve-metadata=entitlements "$framework/Versions/B/XPCServices/Downloader.xpc"
sign "$framework/Versions/B/Autoupdate"
sign "$framework/Versions/B/Updater.app"
sign "$framework"
sign "$app"
codesign --verify --strict --deep "$app"

/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app" || true
echo "$app ($(du -sh "$app" | cut -f1))"

if [ "${1:-}" = "--open" ] && [ -n "${2:-}" ]; then
	open -a "$PWD/$app" "$2"
fi
