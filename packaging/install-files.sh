# install-files.sh ROOT BINARY: lays out the installed files under ROOT, for
# every package. Run from the unpacked source tree.
set -eu
root=$1
install -Dm755 "$2" "$root/usr/bin/poe-overlay"
install -Dm644 packaging/io.github.nephritus.PoeOverlay.Settings.desktop -t "$root/usr/share/applications"
install -Dm644 packaging/io.github.nephritus.PoeOverlay.svg -t "$root/usr/share/icons/hicolor/scalable/apps"
install -Dm644 packaging/60-poe-overlay-uinput.rules -t "$root/usr/lib/udev/rules.d"
