# 17: Release v0.2.0

**What to build:** Cargo.toml and the packages say 0.2.0; README describes the Menu, Features, the theme and the tray; the local `packaging/release.sh` passes; tag `v0.2.0` is pushed and the workflow publishes the release with its files and the PKGBUILD.

**Blocked by:** 02, 04, 05, 06, 07, 08, 09, 10, 11, 12, 13, 14, 15, 16

**Status:** ready-for-agent

- [ ] `packaging/release.sh` passes all smoke containers with the fonts present
- [ ] The GitHub release v0.2.0 has the .deb, .rpm, AppImage, tarball, SHA256SUMS and PKGBUILD
- [ ] README matches the shipped behaviour
