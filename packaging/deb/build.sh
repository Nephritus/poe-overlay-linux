# Builds dist/poe-overlay_$VERSION-1_amd64.deb in Debian 13 (see release.sh).
set -eu
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends ca-certificates curl gcc libc6-dev pkg-config dpkg-dev \
	libgtk-4-dev libgtk4-layer-shell-dev libxkbcommon-dev libsqlite3-dev >/dev/null
. /packaging/rust.sh

cargo build --locked --release

root=/build/deb
sh packaging/install-files.sh "$root" target/release/poe-overlay
install -Dm644 LICENSE-MIT LICENSE-APACHE -t "$root/usr/share/doc/poe-overlay"
install -Dm644 data/exile-leveling/LICENSE "$root/usr/share/doc/poe-overlay/LICENSE-exile-leveling"
cat >"$root/usr/share/doc/poe-overlay/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: poe-overlay
Source: https://github.com/nephritus/poe-overlay-linux

Files: *
Copyright: 2026 Nephritus
License: Expat or Apache-2.0
 See LICENSE-MIT and LICENSE-APACHE in this folder.

Files: data/exile-leveling/*
Copyright: HeartofPhos
License: Expat
 See LICENSE-exile-leveling in this folder.
EOF
cat <<EOF | gzip -9n >"$root/usr/share/doc/poe-overlay/changelog.Debian.gz"
poe-overlay ($VERSION-1) unstable; urgency=medium

  * Release $VERSION: https://github.com/nephritus/poe-overlay-linux/releases/tag/v$VERSION

 -- Nephritus <90871716+Nephritus@users.noreply.github.com>  $(date -R -u -d "@$SOURCE_DATE_EPOCH")
EOF

# The libraries the program links, as Debian package dependencies.
mkdir -p /build/shlibs/debian
printf 'Source: poe-overlay\n\nPackage: poe-overlay\nArchitecture: amd64\n' >/build/shlibs/debian/control
depends=$(cd /build/shlibs && dpkg-shlibdeps -O "$root/usr/bin/poe-overlay" | sed -n 's/^shlibs:Depends=//p')

mkdir -p "$root/DEBIAN"
cat >"$root/DEBIAN/control" <<EOF
Package: poe-overlay
Version: $VERSION-1
Architecture: amd64
Maintainer: Nephritus <90871716+Nephritus@users.noreply.github.com>
Installed-Size: $(du -sk "$root/usr" | cut -f1)
Depends: $depends
Recommends: xwayland
Section: games
Priority: optional
Homepage: https://github.com/nephritus/poe-overlay-linux
Description: Path of Exile 1 overlay for Wayland
 Leveling Route, Price Check, Currency Rates and Trade Requests over the game,
 shown only while it's focused. Start it with the Steam launch option
 "poe-overlay launch -- %command%".
EOF
cp packaging/udev-reload.sh "$root/DEBIAN/postinst"
chmod 755 "$root/DEBIAN/postinst"

dpkg-deb --root-owner-group -Zxz --build "$root" "/dist/poe-overlay_${VERSION}-1_amd64.deb"
