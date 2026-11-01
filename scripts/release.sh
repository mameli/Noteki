#!/bin/bash
# Builds Noteki in Release, signs it with the "Developer ID Application" certificate, puts it in a
# DMG with a link to Applications, has Apple notarize the DMG (notarytool profile "noteki", else
# "takku" or "dimmela" if one of those is on this Mac, or $NOTEKI_NOTARY_PROFILE), staples the
# ticket and publishes it as a GitHub Release tagged v<MARKETING_VERSION> (from
# Config/Base.xcconfig). The Release also carries a copy named Noteki.dmg, so
# releases/latest/download/Noteki.dmg always gives the latest version. Adapted from Dimmela's script.
#
#   scripts/release.sh            build, sign, notarize into build/release/, then ask before publishing
#   scripts/release.sh --dry-run  build, sign, notarize only
#
# Once per Mac: the certificate from Xcode → Settings → Accounts → Manage Certificates, and
#   xcrun notarytool store-credentials noteki --apple-id <Apple ID> --team-id <Team ID>
# with an app-specific password from account.apple.com.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n 's/^MARKETING_VERSION = //p' Config/Base.xcconfig)
tag="v$version"
app=build/release/DerivedData/Build/Products/Release/Noteki.app
dmg="build/release/Noteki-$version.dmg"
latest=build/release/Noteki.dmg
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

rm -rf build/release
# No get-task-allow entitlement: it is only for debugging (and notarization refuses it).
# A secure timestamp is required by notarization.
xcodebuild -project Noteki.xcodeproj -scheme Noteki -configuration Release \
    -derivedDataPath build/release/DerivedData \
    CODE_SIGN_STYLE=Manual CODE_SIGN_IDENTITY="$identity" DEVELOPMENT_TEAM="$team" \
    CODE_SIGN_INJECT_BASE_ENTITLEMENTS=NO OTHER_CODE_SIGN_FLAGS=--timestamp \
    clean build | grep -E "error: |BUILD"

codesign --verify --strict --deep "$app"
[[ $(codesign -dvv "$app" 2>&1) == *"Authority=$identity"* ]] || { echo "Not signed with the $identity certificate." >&2; exit 1; }

# The DMG: the app and a link to Applications, to drag it there.
staging=build/release/dmg
mkdir -p "$staging"
ditto "$app" "$staging/Noteki.app"
ln -s /Applications "$staging/Applications"
hdiutil create -quiet -volname "Noteki $version" -srcfolder "$staging" -fs HFS+ -format UDZO "$dmg"
codesign --sign "$identity" --timestamp "$dmg"

# The DMG is notarized and its ticket stapled, so Gatekeeper accepts it offline too; the app
# inside is covered by the same ticket.
xcrun notarytool submit "$dmg" --keychain-profile "$notary" --wait --output-format json > build/release/notary.json || true
if [[ $(plutil -extract status raw -o - build/release/notary.json 2>/dev/null) != Accepted ]]; then
    cat build/release/notary.json >&2
    id=$(plutil -extract id raw -o - build/release/notary.json 2>/dev/null)
    [[ -n $id ]] && xcrun notarytool log "$id" --keychain-profile "$notary" >&2
    echo "Notarization failed." >&2
    exit 1
fi
xcrun stapler staple -q "$dmg"
spctl --assess --type open --context context:primary-signature "$dmg" || { echo "Gatekeeper refuses the notarized DMG." >&2; exit 1; }
cp "$dmg" "$latest"
echo "Ready: $dmg (notarized)"

[[ ${1:-} == --dry-run ]] && exit 0
read -r -p "Publish $tag on GitHub? [y/N] " answer
[[ $answer == y ]] || exit 0

if [[ $(git rev-parse HEAD) != $(gh api "repos/{owner}/{repo}/commits/main" --jq .sha) ]]; then
    echo "Push main first: the tag is created on the commit GitHub has." >&2
    exit 1
fi
gh release create "$tag" "$dmg" "$latest" --target main --title "Noteki $version" --notes-file - <<NOTES
Mac with macOS 15 or later.

Download \`Noteki-$version.dmg\` (\`Noteki.dmg\` is the same file), open it and drag **Noteki** to **Applications**. Noteki is notarized by Apple: it opens like any other app.

What's in it: see [CHANGELOG.md](https://github.com/mameli/Noteki/blob/$tag/CHANGELOG.md).
NOTES
echo "Published: $(gh release view "$tag" --json url --jq .url)"
