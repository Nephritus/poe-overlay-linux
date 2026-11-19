# 16: Packaging: AUR, cargo install, AppImage, .deb, .rpm

**What to build:** The companion can be installed via AUR, `cargo install`, AppImage, .deb and .rpm (no Flatpak: its sandbox blocks uinput, the Steam folder and the trigger socket). Done early so toolkit-specific packaging problems (e.g. bundling GTK4 and gtk4-layer-shell in an AppImage, where load order matters) surface before features pile up. README carries GGG's "This product isn't affiliated with or endorsed by Grinding Gear Games in any way." notice, the uinput udev rule, compositor requirements and the Steam launch option.

**Blocked by:** 02 (Companion skeleton)

**Status:** ready-for-human

- [x] Each of the five install methods produces a working `poe-overlay` that passes the ticket 02 checks
- [x] .deb and .rpm declare their runtime dependencies; the AppImage runs on a system without the toolkit installed
- [x] Release builds of all artifacts are reproducible from one documented command or CI workflow
- [x] README contains the GGG notice, udev rule, compositor requirements and launch option

## Comments

### Implemented (agent, 2026-11-19)

User's choices (form): MIT OR Apache-2.0; one podman script, also run by GitHub Actions; an AppImage bundling everything down to glibc (quick-sharun / sharun from Anylinux); the packages install the uinput udev rule. Author from git: Nephritus.

- **Targets** (checked 2026-11-19): gtk4-layer-shell is in Debian 13 (1.0.4), Ubuntu 25.04+ (26.04: 1.3) and Fedora 43 (1.3); **Ubuntu 24.04 has none**, so the README sends it to the AppImage.
- **`packaging/release.sh`** builds into `dist/`: the source tarball (`git archive`), the .deb in Debian 13 (Depends from `dpkg-shlibdeps`), the .rpm in Fedora 43 (automatic Requires plus `libGLESv2.so.2`: Fedora's minimal GTK doesn't pull it in and GTK aborts without it, seen live), the AppImage in Arch with quick-sharun and Anylinux's LLVM-free Mesa, and `dist/aur/PKGBUILD` + `.SRCINFO` with the tarball's checksum. Then `smoke.sh` installs each into clean containers. Images are pinned by digest, Rust 1.99.1, quick-sharun by commit and checksum, times from the last commit; distro packages come from the mirrors on the day, so rebuilds match in content, not byte for byte. `.github/workflows/release.yml` runs the same script on a `v*` tag (tag must match Cargo.toml) and attaches the artifacts; untested, since the repo has no remote yet.
- **The AppImage and the game's environment:** sharun points `GTK_PATH`, `XDG_DATA_DIRS`, `LD_PRELOAD`, `__EGL_VENDOR_LIBRARY_FILENAMES` and more into the AppImage, and `launch` execs the game, so Proton would inherit them. Our `AppRun` snapshots the environment it was started with (`/proc/$$/environ`, base64) into `POE_OVERLAY_HOST_ENV`; `src/host_env.rs` gives that environment, minus the AppImage runtime's own variables, to the game, to re-runs of the program (through `$APPIMAGE`, since the mount dies with the process that made it) and to the browser opened for login. Outside the AppImage nothing changes. `base64` moved from `trade_search.rs` to a shared `src/base64.rs`.
- **Tests:** `tests/launch.rs` `from_the_appimage_the_game_gets_the_environment_steam_gave_it`: the game gets exactly the snapshot (multi-line value, base64 `+` `/` `=`), the overlay starts through `$APPIMAGE` without Steam's preload or the AppImage's GTK paths. Every mutation of `host_env`/`base64` it covers was killed.
- **Clean-container checks** (`packaging/smoke-checks.sh`, as an ordinary user, under a headless sway): version, credits, every library found (`ldd`), `launch` passing exit code 7 through, the overlay starting beside the game (GTK and layer-shell up), one overlay for two launches, none after the games end; for the AppImage also the game's environment equal to the shell's. All pass:

| Method | Container |
|---|---|
| .deb | Debian 13, Ubuntu 26.04 |
| .rpm | Fedora 43 |
| AppImage (no GTK installed) | Ubuntu 24.04, Fedora 43 |
| AUR (`makepkg` from the tarball, checksum checked, `check()` runs the test suite) | Arch Linux |
| `cargo install` of the packaged crate (only the manifest's `include`) | Debian 13 |

- **Checked live on niri** (stand-in window titled "Path of Exile"): the .deb (Debian 13 container) and the .rpm (Fedora 43 container) on the host's Wayland socket, and the AppImage on the host (FUSE mount): the Route Panel showed while the stand-in was focused and hid when it wasn't; the overlay exited with the game. Under the real AppImage the game's environment equalled the session's, Steam's `LD_PRELOAD` included.
- **Also found:** makepkg's LTO breaks linking `ring` (`!lto`, explained); Ubuntu 24.04's sway won't start beside NVIDIA's driver; Fedora's sway has file capabilities a container can't grant.
- **Beyond the ticket, small:** a "PoE Overlay Settings" menu entry and icon (the AppImage needs a desktop entry; the packages ship it), udev rule reload after install.
- **After review:** AUR checksum filled in at release (no `SKIP`), the overlay's start and lifetime checked in every container (was CLI only), images pinned by digest and the reproducibility claim made honest, package scripts taken from the tarball only, `%udev_rules_update`, Debian `Expat` and changelog, version-free README names. Not changed: `host_env` copies the decoded environment once into owned strings.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Publish | Push the repo to GitHub, tag `v0.1.0`: the workflow attaches the files; push `dist/aur/*` to the AUR | GitHub: done 2026-11-19 (repo created, release v0.1.0; the workflow's upload step failed on `dist/aur`, fixed in 421f590, so the release was attached by hand from that run's artifacts, with the filled-in `PKGBUILD`). AUR: pending, registration closed |
| AppImage from Steam | Launch option `~/Applications/poe-overlay.AppImage launch -- %command%`: PoE starts, the overlay shows in game, the Steam overlay (Shift+Tab) still works | |
| A package from Steam | Same with an installed package's `poe-overlay launch -- %command%` on Debian/Ubuntu or Fedora | |
