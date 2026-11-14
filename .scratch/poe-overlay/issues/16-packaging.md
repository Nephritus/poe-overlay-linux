# 16: Packaging: AUR, cargo install, AppImage, .deb, .rpm

**What to build:** The companion can be installed via AUR, `cargo install`, AppImage, .deb and .rpm (no Flatpak: its sandbox blocks uinput, the Steam folder and the trigger socket). Done early so toolkit-specific packaging problems (e.g. bundling GTK4 and gtk4-layer-shell in an AppImage, where load order matters) surface before features pile up. README carries GGG's "This product isn't affiliated with or endorsed by Grinding Gear Games in any way." notice, the uinput udev rule, compositor requirements and the Steam launch option.

**Blocked by:** 02 (Companion skeleton)

**Status:** ready-for-agent

- [ ] Each of the five install methods produces a working `poe-overlay` that passes the ticket 02 checks
- [ ] .deb and .rpm declare their runtime dependencies; the AppImage runs on a system without the toolkit installed
- [ ] Release builds of all artifacts are reproducible from one documented command or CI workflow
- [ ] README contains the GGG notice, udev rule, compositor requirements and launch option
