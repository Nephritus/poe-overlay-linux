# Installs each artifact in dist/ into a clean container, adds sway for a
# headless compositor, and runs packaging/smoke-checks.sh there as an ordinary
# user (run by release.sh, which sets ENGINE and the images).
set -eu
UBUNTU=docker.io/library/ubuntu:26.04@sha256:0d5255be560e2d2ddcfbfda88549e9e815ef29e7cd7fc62183d5709e6e2963c6
UBUNTU_LTS=docker.io/library/ubuntu:24.04@sha256:77eb101c607d6e3d4f5ad0c8a02c2136c689b98aba1f359e6502ba517e5c57ec

# smoke IMAGE LABEL INSTALL-COMMANDS [CHECK-ENV]
smoke() {
	echo "-- $2 in $1"
	"$ENGINE" run --rm -e VERSION -e RUST_VERSION -v "$PWD/dist:/dist:ro" -v "$PWD/packaging:/packaging:ro" \
		-v poe-overlay-cargo:/root/.cargo/registry "$1" sh -c "set -eu
$3
# Fedora's sway carries file capabilities, which an unprivileged container
# can't grant; a copy runs without them.
cp \"\$(command -v sway)\" /usr/local/bin/sway
useradd -m smoke
su smoke -c \"PATH=\$PATH VERSION=$VERSION ${4:-} sh /packaging/smoke-checks.sh\""
}

deb=/dist/poe-overlay_${VERSION}-1_amd64.deb
apt='export DEBIAN_FRONTEND=noninteractive; apt-get update -qq && apt-get install -y -qq --no-install-recommends'
smoke "$DEBIAN" .deb "$apt $deb sway >/dev/null"
smoke "$UBUNTU" .deb "$apt $deb sway >/dev/null"

# Fedora's image has no su: util-linux brings it.
smoke "$FEDORA" .rpm "dnf install -y -q /dist/poe-overlay-$VERSION-1.*.x86_64.rpm sway util-linux >/dev/null"

# No GTK in these: everything comes from the AppImage. No FUSE in a container,
# so it unpacks itself instead of mounting.
appimage="printf '#!/bin/sh\nexec /dist/poe-overlay-$VERSION-x86_64.AppImage \"\$@\"\n' >/usr/local/bin/poe-overlay
chmod +x /usr/local/bin/poe-overlay"
smoke "$UBUNTU_LTS" AppImage "$apt sway >/dev/null
$appimage" 'APPIMAGE_TEST=1 APPIMAGE_EXTRACT_AND_RUN=1'
smoke "$FEDORA" AppImage "dnf install -y -q sway util-linux >/dev/null
$appimage" 'APPIMAGE_TEST=1 APPIMAGE_EXTRACT_AND_RUN=1'

# The AUR package release.sh filled in, built as makepkg would from the
# source tarball, checksum checked.
smoke "$ARCH" AUR "pacman -Syu --noconfirm --needed base-devel rust gtk4 gtk4-layer-shell libxkbcommon sqlite sway >/dev/null
useradd -m builder
cp /dist/aur/PKGBUILD /dist/poe-overlay-$VERSION.tar.gz /home/builder/
chown -R builder /home/builder
su builder -c 'cd ~ && makepkg --noconfirm >/tmp/makepkg.log 2>&1' || { tail -40 /tmp/makepkg.log; exit 1; }
pacman -U --noconfirm /home/builder/poe-overlay-$VERSION-1-x86_64.pkg.tar.zst >/dev/null"

# cargo install of the crate as it would be published (only the files the
# manifest includes).
smoke "$DEBIAN" 'cargo install' "$apt ca-certificates curl gcc libc6-dev pkg-config \
	libgtk-4-dev libgtk4-layer-shell-dev libxkbcommon-dev libsqlite3-dev sway >/dev/null
. /packaging/rust.sh
cargo package --locked --no-verify -q
mkdir /crate && tar -xzf target/package/poe-overlay-$VERSION.crate -C /crate
cargo install --locked -q --path /crate/poe-overlay-$VERSION --root /usr/local"
