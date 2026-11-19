#!/bin/bash
# Builds Noteki (scripts/bundle.sh) signed with the "Developer ID Application"
# certificate, Sparkle's helpers too, with the hardened runtime; has Apple
# notarize it (notarytool profile "noteki", else "takku" or "dimmela" if one of
# those is on this Mac, or $NOTEKI_NOTARY_PROFILE) and staples the ticket;
# packs it into the disk image to drag into Applications (scripts/dmg),
# signed, notarized and stapled too; writes appcast.xml, signed with the
# Sparkle key; and publishes them as the GitHub Release v<version> (Cargo.toml),
# its notes the version's section of CHANGELOG.md, with a copy named
# Noteki.dmg (releases/latest/download/Noteki.dmg is always the latest). Every
# Noteki, the Swift ones of 0.3 too (same bundle id and key), reads that
# appcast and updates itself. Then the Homebrew cask (a clone of mameli/homebrew-noteki,
# ../homebrew-noteki or $NOTEKI_TAP) points to it.
#
#   scripts/release.sh            build, sign, notarize into target/release-dmg/, then ask before publishing
#   scripts/release.sh --dry-run  build, sign, notarize only
#
# Once per Mac: the certificate from Xcode → Settings → Accounts → Manage Certificates, and
#   xcrun notarytool store-credentials noteki --apple-id <Apple ID> --team-id <Team ID>
# The Sparkle signing key is in the keychain (account "noteki"; back it up with
# generate_keys --account noteki -x <file>): without it no Noteki can install the update.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
build=$(sed -n 's/^build = //p' Cargo.toml | head -1)
minimum_macos=$(sed -n 's:.*<key>LSMinimumSystemVersion</key><string>\(.*\)</string>.*:\1:p' scripts/bundle.sh)
tag="v$version"
out=target/release-dmg
app=target/release/Noteki.app
dmg="$out/Noteki-$version.dmg"
latest="$out/Noteki.dmg"
zip="$out/Noteki.zip"
appcast="$out/appcast.xml"
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

if [[ $(uname -m) != arm64 ]]; then
    echo "Build on an Apple Silicon Mac: the release is for arm64." >&2
    exit 1
fi
if [[ -n $(git status --porcelain) ]]; then
    echo "Commit or stash your changes first: the Release must match a commit." >&2
    exit 1
fi
if gh release view "$tag" >/dev/null 2>&1; then
    echo "Release $tag already exists: raise the version in Cargo.toml." >&2
    exit 1
fi
# Sparkle compares the build numbers: each release must raise it.
latest_build=$(curl -fsL https://github.com/mameli/Noteki/releases/latest/download/appcast.xml 2>/dev/null \
    | sed -n 's:.*<sparkle\:version>\(.*\)</sparkle\:version>.*:\1:p' || true)
if [[ -n $latest_build && $build -le $latest_build ]]; then
    echo "The build is $build, the latest release's is $latest_build: raise [package.metadata.noteki] build in Cargo.toml." >&2
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

rm -rf "$out"
mkdir -p "$out"
NOTEKI_IDENTITY="$identity" NOTEKI_PROFILE=dist scripts/bundle.sh
[[ $(codesign -dvv "$app" 2>&1) == *"Authority=$identity"* ]] || { echo "Not signed with the $identity certificate." >&2; exit 1; }
[[ $(lipo -archs "$app/Contents/MacOS/noteki") == arm64 ]] || { echo "The binary is not arm64." >&2; exit 1; }
sparkle_bin=$(ls -d target/sparkle-*/bin | tail -1)

notarize() {
    xcrun notarytool submit "$1" --keychain-profile "$notary" --wait --output-format json > "$out/notary.json" || true
    if [[ $(plutil -extract status raw -o - "$out/notary.json" 2>/dev/null) != Accepted ]]; then
        cat "$out/notary.json" >&2
        id=$(plutil -extract id raw -o - "$out/notary.json" 2>/dev/null)
        [[ -n $id ]] && xcrun notarytool log "$id" --keychain-profile "$notary" >&2
        echo "Notarization of $1 failed." >&2
        exit 1
    fi
}

# The app is notarized as a zip and its ticket stapled, so Gatekeeper accepts
# it offline once in Applications, or installed by Sparkle. Then the disk
# image around it is signed, notarized and stapled.
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

# The appcast Sparkle reads: this version only, with what's new and the disk
# image's EdDSA signature. Made in the dry run too, so a missing key shows
# before publishing.
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
read -r -p "Publish $tag on GitHub? Every Noteki will offer it as an update. [y/N] " answer
[[ $answer == y ]] || exit 0

# The tag is made on the commit GitHub has.
head=$(git rev-parse HEAD)
branch=$(git rev-parse --abbrev-ref HEAD)
if [[ $head != $(gh api "repos/{owner}/{repo}/commits/$branch" --jq .sha 2>/dev/null) ]]; then
    echo "Push $branch first: the tag is created on the commit GitHub has." >&2
    exit 1
fi
gh release create "$tag" "$dmg" "$latest" "$appcast" --target "$head" --title "Noteki $version" --notes-file - <<NOTES
Apple Silicon Mac with macOS $minimum_macos or later.

Download \`Noteki-$version.dmg\` (\`Noteki.dmg\` is the same file), open it and drag **Noteki** to **Applications**. Noteki is notarized by Apple: it opens like any other app. With Homebrew: \`brew tap mameli/noteki && brew trust mameli/noteki && brew install --cask noteki\`.

Noteki updates itself: *Check for Updates…* in the Noteki menu, then **Install Update**.

## What's new

$changes

Every version: [CHANGELOG.md](https://github.com/mameli/Noteki/blob/$tag/CHANGELOG.md).
NOTES
echo "Published: $(gh release view "$tag" --json url --jq .url)"

# The cask points to this version's disk image; Noteki updates itself
# (auto_updates), so brew upgrade leaves it to Noteki.
sha=$(shasum -a 256 "$dmg" | cut -d ' ' -f 1)
git -C "$tap" pull --ff-only -q
sed -i '' -e "s/^  version \".*\"/  version \"$version\"/" -e "s/^  sha256 \".*\"/  sha256 \"$sha\"/" "$cask"
git -C "$tap" commit -q -am "Noteki $version"
git -C "$tap" -c credential.helper= -c 'credential.helper=!gh auth git-credential' push -q origin main
echo "Homebrew cask updated to $version."
