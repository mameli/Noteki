#!/bin/bash
# Packs Noteki.app into a disk image: the app and a link to Applications on the background drawn by
# scripts/dmg/background.py, with Noteki's icon on the volume. The window layout is the .DS_Store
# saved in scripts/dmg/DS_Store, so packing needs neither Finder nor a free /Volumes/Noteki.
# Signing and notarization are up to the caller (release.sh). From Takku's and Dimmela's.
#
#   scripts/dmg/make-dmg.sh <Noteki.app> <Noteki.dmg>
#   scripts/dmg/make-dmg.sh --layout
#
# --layout writes scripts/dmg/DS_Store again, after changing the background or the positions:
# dmgbuild (installed in build.noindex/dmgbuild, once) lays the window out without Finder, so no
# window opens and the terminal needs no permission to control Finder.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)

# Positions and size match scripts/dmg/background.py (640 × 400; icons at ICON_Y, NOTEKI_X,
# APPLICATIONS_X).
if [[ ${1:-} == --layout ]]; then
    venv=$root/build.noindex/dmgbuild
    [[ -x $venv/bin/dmgbuild ]] || { python3 -m venv "$venv" && "$venv/bin/pip" install --quiet dmgbuild; }
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    # A stand-in app: only its name matters to the layout.
    mkdir -p "$work/Noteki.app"
    cat > "$work/settings.py" <<SETTINGS
files = ["$work/Noteki.app"]
symlinks = {"Applications": "/Applications"}
background = "$here/background.tiff"
window_rect = ((200, 120), (640, 400))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
icon_size = 128
text_size = 13
arrange_by = None
icon_locations = {"Noteki.app": (175, 200), "Applications": (465, 200)}
SETTINGS
    "$venv/bin/dmgbuild" -s "$work/settings.py" Noteki "$work/layout.dmg" >/dev/null
    hdiutil attach "$work/layout.dmg" -readonly -noverify -noautoopen -nobrowse -quiet -mountpoint "$work/mnt"
    cp "$work/mnt/.DS_Store" "$here/DS_Store"
    hdiutil detach -quiet "$work/mnt"
    echo "Window layout saved in scripts/dmg/DS_Store."
    exit 0
fi

app=$1
out=$2
work=$(mktemp -d)
volume=$work/mnt
trap 'hdiutil detach -quiet -force "$volume" 2>/dev/null || true; rm -rf "$work"' EXIT
mkdir -p "$work/src"
ditto "$app" "$work/src/Noteki.app"
ln -s /Applications "$work/src/Applications"
# Where the layout's alias looks for it: at the volume's root, hidden.
cp "$here/background.tiff" "$work/src/.background.tiff"
cp "$app/Contents/Resources/AppIcon.icns" "$work/src/.VolumeIcon.icns"
cp "$here/DS_Store" "$work/src/.DS_Store"

hdiutil create -volname Noteki -srcfolder "$work/src" -fs HFS+ -format UDRW -ov "$work/rw.dmg" -quiet
mkdir "$volume"
hdiutil attach "$work/rw.dmg" -readwrite -noverify -noautoopen -nobrowse -quiet -mountpoint "$volume"
# The volume shows its own icon.
xcrun SetFile -a C "$volume"
sync
hdiutil detach -quiet "$volume"
rm -f "$out"
# LZMA (ULMO), read since macOS 10.15: a quarter smaller than zlib's UDZO.
hdiutil convert "$work/rw.dmg" -format ULMO -o "$out" -quiet
