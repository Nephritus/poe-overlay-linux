#!/bin/sh
# Builds the release artifacts into dist/, each in a container:
#   poe-overlay-VERSION.tar.gz             source, for the AUR and cargo
#   poe-overlay_VERSION-1_amd64.deb        Debian 13+, Ubuntu 25.04+
#   poe-overlay-VERSION-1.fcNN.x86_64.rpm  Fedora 43+
#   poe-overlay-VERSION-x86_64.AppImage    any distro, GTK bundled
#   aur/PKGBUILD, aur/.SRCINFO             for the AUR, checksum filled in
# then installs each into a clean container and checks it there (smoke.sh).
#
# Usage: packaging/release.sh [source|deb|rpm|appimage|aur|smoke]...  (default: all)
# Needs git and podman (CONTAINER_ENGINE=docker works too). It builds the
# committed tree; uncommitted changes to tracked files are included, with a
# warning. File times come from the last commit (SOURCE_DATE_EPOCH); the
# images (by digest) and the Rust toolchain are pinned, the distro packages
# are the mirrors' current ones.
set -eu
cd "$(dirname "$0")/.."

ENGINE=${CONTAINER_ENGINE:-podman}
export RUST_VERSION=1.99.1
export VERSION; VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
export SOURCE_DATE_EPOCH; SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)
DEBIAN=docker.io/library/debian:13@sha256:bfc36aecdece0387418759dba474aa856c026ca8087da3606b41b90ab0509c99
FEDORA=registry.fedoraproject.org/fedora:43@sha256:40e048329e89502dae22f4f35d44aaca50bffb611e5c0c9d50bbbe7a5092db9b
ARCH=docker.io/library/archlinux:base-20261108.0.623074@sha256:fcee73ea1b96e236248f53a213f71276cc9072d9025b58bce9915e32f19963ae

steps=${*:-source deb rpm appimage aur smoke}
mkdir -p dist

# Runs packaging/$2 in image $1 with dist/ and packaging/ mounted, and a
# shared cargo download cache.
run() {
	"$ENGINE" run --rm -e VERSION -e RUST_VERSION -e SOURCE_DATE_EPOCH \
		-v "$PWD/dist:/dist" -v "$PWD/packaging:/packaging:ro" \
		-v poe-overlay-cargo:/root/.cargo/registry "$1" sh "/packaging/$2"
}

for step in $steps; do
	echo "== $step"
	case $step in
	source)
		ref=$(git stash create)
		if [ -n "$ref" ]; then
			echo "warning: building uncommitted changes" >&2
		fi
		git archive --format=tar --prefix="poe-overlay-$VERSION/" "${ref:-HEAD}" | gzip -9n >"dist/poe-overlay-$VERSION.tar.gz"
		;;
	deb) run "$DEBIAN" deb/build.sh ;;
	rpm) run "$FEDORA" rpm/build.sh ;;
	appimage) run "$ARCH" appimage/build.sh ;;
	aur)
		sum=$(sha256sum "dist/poe-overlay-$VERSION.tar.gz" | cut -d ' ' -f 1)
		mkdir -p dist/aur
		sed -e "s/@VERSION@/$VERSION/" -e "s/@SHA256@/$sum/" packaging/aur/PKGBUILD >dist/aur/PKGBUILD
		# makepkg won't run as root.
		"$ENGINE" run --rm -v "$PWD/dist/aur:/aur:ro" "$ARCH" sh -c \
			'useradd -m builder && cp /aur/PKGBUILD /home/builder/ && cd /home/builder && su builder -c "makepkg --printsrcinfo"' \
			>dist/aur/.SRCINFO
		;;
	smoke) DEBIAN=$DEBIAN FEDORA=$FEDORA ARCH=$ARCH ENGINE=$ENGINE sh packaging/smoke.sh ;;
	*) echo "unknown step: $step" >&2; exit 2 ;;
	esac
done
(cd dist && sha256sum ./*.tar.gz ./*.deb ./*.rpm ./*.AppImage 2>/dev/null >SHA256SUMS) || :
ls -l dist
