#!/bin/bash
# Builds Noteki in Release, signs it with the "Developer ID Application" certificate (Sparkle's
# helpers too), has Apple notarize the app (notarytool profile "noteki", else "takku" or "dimmela"
# if one of those is on this Mac, or $NOTEKI_NOTARY_PROFILE) and staples the ticket, packs it into
# a disk image with a background to drag it into Applications (scripts/dmg/make-dmg.sh), signs,
# notarizes and staples that too, and publishes it as a GitHub Release tagged v<MARKETING_VERSION>
# (from Config/Base.xcconfig), its notes the version's section of CHANGELOG.md. The Release also
# carries a copy named Noteki.dmg, so releases/latest/download/Noteki.dmg always gives the latest
# version, and appcast.xml: Noteki reads releases/latest/download/appcast.xml with Sparkle and
# installs the disk image after checking its EdDSA signature (ADR 0011). Then it points the
# Homebrew cask in the tap (a clone of mameli/homebrew-noteki, ../homebrew-noteki or $NOTEKI_TAP)
# to it. Adapted from Dimmela's.
#
#   scripts/release.sh            build, sign, notarize into build.noindex/release/, then ask before publishing
#   scripts/release.sh --dry-run  build, sign, notarize only
#
# Once per Mac: the certificate from Xcode → Settings → Accounts → Manage Certificates, and
#   xcrun notarytool store-credentials noteki --apple-id <Apple ID> --team-id <Team ID>
# with an app-specific password from account.apple.com. The Sparkle signing key is in the
# keychain too (account "noteki", made with Sparkle's generate_keys; back it up with
# generate_keys --account noteki -x <file>): without it no Noteki can install the update.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n 's/^MARKETING_VERSION = //p' Config/Base.xcconfig)
build=$(sed -n 's/^CURRENT_PROJECT_VERSION = //p' Config/Base.xcconfig)
minimum_macos=$(sed -n 's/^MACOSX_DEPLOYMENT_TARGET = //p' Config/Base.xcconfig)
tag="v$version"
app=build.noindex/release/DerivedData/Build/Products/Release/Noteki.app
dmg="build.noindex/release/Noteki-$version.dmg"
latest=build.noindex/release/Noteki.dmg
zip=build.noindex/release/Noteki.zip
appcast=build.noindex/release/appcast.xml
sparkle_bin=build.noindex/release/DerivedData/SourcePackages/artifacts/sparkle/Sparkle/bin
tap=${NOTEKI_TAP:-../homebrew-noteki}
cask="$tap/Casks/noteki.rb"
identity="Developer ID Application"
notary=${NOTEKI_NOTARY_PROFILE:-}
# The credentials are the same Apple ID and team: a profile made for another app works too.
if [[ -z $notary ]]; then
    for profile in noteki takku dimmela; do
        if xcrun notarytool history --keychain-profile "$profile" >/dev/null 2>&1; then
            notary=$profile
            break
        fi
    done
fi

if [[ -n $(git status --porcelain) ]]; then
    echo "Commit or stash your changes first: the Release must match a commit." >&2
    exit 1
fi
if gh release view "$tag" >/dev/null 2>&1; then
    echo "Release $tag already exists: raise MARKETING_VERSION in Config/Base.xcconfig." >&2
    exit 1
fi
# Sparkle compares the build numbers: each release must raise it.
latest_build=$(curl -fsL https://github.com/mameli/Noteki/releases/latest/download/appcast.xml 2>/dev/null \
    | sed -n 's:.*<sparkle\:version>\(.*\)</sparkle\:version>.*:\1:p' || true)
if [[ -n $latest_build && $build -le $latest_build ]]; then
    echo "CURRENT_PROJECT_VERSION is $build, the latest release's is $latest_build: raise it in Config/Base.xcconfig." >&2
    exit 1
fi
if [[ ! -f $cask || -n $(git -C "$tap" status --porcelain) ]]; then
    echo "The Homebrew tap must be a clean clone of mameli/homebrew-noteki in $tap (or set NOTEKI_TAP)." >&2
    exit 1
fi
changes=$(scripts/changelog-section "$version")
if [[ -z $changes ]]; then
    echo "CHANGELOG.md has no \"## $version\" section." >&2
    exit 1
fi

# The Team ID is the certificate's organizational unit.
team=$(security find-certificate -c "$identity" -p 2>/dev/null | openssl x509 -noout -subject -nameopt multiline 2>/dev/null \
    | sed -n 's/^ *organizationalUnitName *= *//p')
if [[ -z $team ]]; then
    echo "No \"$identity\" certificate in the keychain: create it in Xcode → Settings → Accounts → Manage Certificates." >&2
    exit 1
fi
if [[ -z $notary ]]; then
    echo "No notarytool profile: run xcrun notarytool store-credentials noteki --apple-id <Apple ID> --team-id $team" >&2
    exit 1
fi

rm -rf build.noindex/release
# No get-task-allow entitlement: it is only for debugging (and notarization refuses it).
# A secure timestamp is required by notarization.
xcodebuild -project Noteki.xcodeproj -scheme Noteki -configuration Release \
    -derivedDataPath build.noindex/release/DerivedData \
    CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY="$identity" DEVELOPMENT_TEAM="$team" \
    CODE_SIGN_INJECT_BASE_ENTITLEMENTS=NO OTHER_CODE_SIGN_FLAGS=--timestamp \
    clean build | grep -E "error: |BUILD"

# Xcode signs Sparkle.framework but leaves its helpers ad hoc, which notarization refuses: each is
# signed with the Developer ID, inside out, then the framework and the app again.
sparkle=$app/Contents/Frameworks/Sparkle.framework/Versions/B
sign() { codesign --force --sign "$identity" --timestamp --options runtime "$@"; }
sign "$sparkle/XPCServices/Installer.xpc"
sign --preserve-metadata=entitlements "$sparkle/XPCServices/Downloader.xpc"
sign "$sparkle/Autoupdate"
sign "$sparkle/Updater.app"
sign "$app/Contents/Frameworks/Sparkle.framework"
sign --preserve-metadata=entitlements,requirements,flags "$app"

codesign --verify --strict --deep "$app"
[[ $(codesign -dvv "$app" 2>&1) == *"Authority=$identity"* ]] || { echo "Not signed with the $identity certificate." >&2; exit 1; }

notarize() {
    xcrun notarytool submit "$1" --keychain-profile "$notary" --wait --output-format json > build.noindex/release/notary.json || true
    if [[ $(plutil -extract status raw -o - build.noindex/release/notary.json 2>/dev/null) != Accepted ]]; then
        cat build.noindex/release/notary.json >&2
        id=$(plutil -extract id raw -o - build.noindex/release/notary.json 2>/dev/null)
        [[ -n $id ]] && xcrun notarytool log "$id" --keychain-profile "$notary" >&2
        echo "Notarization of $1 failed." >&2
        exit 1
    fi
}

# The app is notarized as a zip and its ticket stapled, so Gatekeeper accepts it offline once it
# is in Applications, or installed by Sparkle. Then the disk image around it is signed, notarized
# and stapled.
ditto -c -k --keepParent "$app" "$zip"
notarize "$zip"
rm "$zip"
xcrun stapler staple -q "$app"
spctl --assess --type execute "$app" || { echo "Gatekeeper refuses the notarized app." >&2; exit 1; }
scripts/dmg/make-dmg.sh "$app" "$dmg"
codesign --sign "$identity" --timestamp "$dmg"
notarize "$dmg"
xcrun stapler staple -q "$dmg"
spctl --assess --type open --context context:primary-signature "$dmg" || { echo "Gatekeeper refuses the notarized DMG." >&2; exit 1; }
cp "$dmg" "$latest"
echo "Ready: $dmg (notarized)"

# The appcast Sparkle reads: this version only, with what's new and the disk image's EdDSA
# signature. Made in the dry-run too, so a missing key shows before publishing.
signature=$("$sparkle_bin/sign_update" --account noteki "$dmg") \
    || { echo "No Sparkle key \"noteki\" in the keychain: see the top of this script." >&2; exit 1; }
notes_html=$(gh api markdown -f mode=gfm -f context=mameli/Noteki -f text="$changes")
cat > "$appcast" <<APPCAST
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Noteki</title>
    <item>
      <title>Noteki $version</title>
      <pubDate>$(LC_ALL=C date -u "+%a, %d %b %Y %H:%M:%S +0000")</pubDate>
      <sparkle:version>$build</sparkle:version>
      <sparkle:shortVersionString>$version</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>$minimum_macos</sparkle:minimumSystemVersion>
      <description><![CDATA[$notes_html]]></description>
      <enclosure url="https://github.com/mameli/Noteki/releases/download/$tag/Noteki-$version.dmg" $signature type="application/octet-stream"/>
    </item>
  </channel>
</rss>
APPCAST
xmllint --noout "$appcast"

[[ ${1:-} == --dry-run ]] && exit 0
read -r -p "Publish $tag on GitHub? [y/N] " answer
[[ $answer == y ]] || exit 0

if [[ $(git rev-parse HEAD) != $(gh api "repos/{owner}/{repo}/commits/main" --jq .sha) ]]; then
    echo "Push main first: the tag is created on the commit GitHub has." >&2
    exit 1
fi
gh release create "$tag" "$dmg" "$latest" "$appcast" --target main --title "Noteki $version" --notes-file - <<NOTES
Mac with macOS 15 or later.

Download \`Noteki-$version.dmg\` (\`Noteki.dmg\` is the same file), open it and drag **Noteki** to **Applications**. Noteki is notarized by Apple: it opens like any other app. With Homebrew: \`brew tap mameli/noteki && brew trust mameli/noteki && brew install --cask noteki\`.

From 0.3.0 on Noteki updates itself: *Check for Updates…* in the Noteki menu, then **Install Update**.

## What's new

$changes

Every version: [CHANGELOG.md](https://github.com/mameli/Noteki/blob/$tag/CHANGELOG.md).
NOTES
echo "Published: $(gh release view "$tag" --json url --jq .url)"

# The cask points to this version's disk image. Noteki updates itself, so the cask says so
# (auto_updates): brew upgrade leaves it to Noteki. Pushed through gh, like the Release.
sha=$(shasum -a 256 "$dmg" | cut -d ' ' -f 1)
git -C "$tap" pull --ff-only -q
sed -i '' -e "s/^  version \".*\"/  version \"$version\"/" -e "s/^  sha256 \".*\"/  sha256 \"$sha\"/" "$cask"
grep -q '^  auto_updates true$' "$cask" || sed -i '' 's/^  depends_on macos: :sequoia$/  auto_updates true\
\
&/' "$cask"
git -C "$tap" commit -q -am "Noteki $version"
git -C "$tap" -c credential.helper= -c 'credential.helper=!gh auth git-credential' push -q origin main
echo "Homebrew cask updated to $version."
