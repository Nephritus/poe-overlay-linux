# Builds dist/poe-overlay-$VERSION-x86_64.AppImage in Arch Linux with
# quick-sharun, which bundles every library down to glibc, so it runs on any
# distro (see release.sh).
set -eu
pacman -Syu --noconfirm --needed base-devel gtk4 gtk4-layer-shell libxkbcommon sqlite patchelf strace >/dev/null
. /packaging/rust.sh

cargo build --locked --release
# quick-sharun deploys from the installed program.
sh packaging/install-files.sh / target/release/poe-overlay

# Pinned: Anylinux-AppImages 9443c15 (2026-11-19).
TOOLS=https://raw.githubusercontent.com/pkgforge-dev/Anylinux-AppImages/9443c1564632280fe6eac00308ad1f81228f14a9/useful-tools
curl -sSfL -o /build/quick-sharun "$TOOLS/quick-sharun.sh"
curl -sSfL -o /build/get-debloated-pkgs "$TOOLS/get-debloated-pkgs.sh"
sha256sum -c - <<EOF
6fc4f4561288297a7f8237f92b872656be1b7a4ba534f2b6cb9cfecabcddfd9d  /build/quick-sharun
d215f7676785f1a1bb8784e69085d66c1df81488ef31f1ba7198325a5be05130  /build/get-debloated-pkgs
EOF
chmod +x /build/quick-sharun /build/get-debloated-pkgs
# Mesa without LLVM, and GTK and glycin trimmed for bundling, from Anylinux's
# rolling package builds (so the AppImage is rebuildable, not bit-identical).
/build/get-debloated-pkgs --add-common --prefer-nano >/dev/null

export APPDIR=/build/AppDir OUTPATH=/dist OUTNAME="poe-overlay-$VERSION-x86_64.AppImage"
export DESKTOP=/usr/share/applications/io.github.nephritus.PoeOverlay.Settings.desktop
export ICON=/usr/share/icons/hicolor/scalable/apps/io.github.nephritus.PoeOverlay.svg
/build/quick-sharun /usr/bin/poe-overlay
# Ours replaces sharun's AppRun, to keep the session's environment.
install -m755 packaging/appimage/AppRun "$APPDIR/AppRun"
rm -f "$APPDIR/AppRun.sh"
/build/quick-sharun --make-appimage
# quick-sharun's notes for its update tooling, not a release file.
rm -f /dist/appinfo
