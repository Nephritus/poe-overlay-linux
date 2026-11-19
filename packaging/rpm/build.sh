# Builds dist/poe-overlay-$VERSION-1.fcNN.x86_64.rpm in Fedora (see release.sh).
set -eu
dnf install -y -q rpm-build gcc pkgconf-pkg-config gtk4-devel gtk4-layer-shell-devel libxkbcommon-devel sqlite-devel >/dev/null
. /packaging/rust.sh

top=/build/rpmbuild
mkdir -p "$top/SOURCES"
cp "/dist/poe-overlay-$VERSION.tar.gz" "$top/SOURCES/"
# File times and build time from SOURCE_DATE_EPOCH, no build host name.
rpmbuild -bb packaging/rpm/poe-overlay.spec \
	--define "_topdir $top" --define "version $VERSION" \
	--define "_buildhost reproducible" \
	--define "use_source_date_epoch_as_buildtime 1" \
	--define "clamp_mtime_to_source_date_epoch 1"
cp "$top"/RPMS/x86_64/poe-overlay-"$VERSION"-1.*.x86_64.rpm /dist/
