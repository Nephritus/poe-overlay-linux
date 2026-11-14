# 17: Hotkeys set up by the overlay on first launch (niri)

**What to build:** The player chooses hotkeys in one place, `config.toml`, instead of hand-editing their compositor config. On the first game launch, the Route Panel shows a one-time line offering to set them up (e.g. "Set up hotkeys? Alt+R: Currency Rates [Set up] [Not now]"). One click writes the binds into the compositor and reloads it, and the answer is remembered so the line doesn't come back.

On niri, the binds go into a file the overlay owns, `~/.config/niri/poe-overlay.kdl`, plus a single `include` line added to the player's `config.kdl`. `poe-overlay setup hotkeys` does the same from a terminal (re-run after changing keys), and `poe-overlay setup hotkeys --remove` deletes the generated file and the include line, nothing else.

Why not the package installer: it runs as root with no user session, can't know which compositor or user, distro policy forbids editing home directories, and `cargo install` and AppImage have no install hook. The launch runs inside the session: the environment Steam passes to launch options carries `XDG_CURRENT_DESKTOP` and `NIRI_SOCKET` (checked 2026-11-14).

Why ask instead of doing it silently like the Wine cursor fix: a hotkey changes the player's compositor config and takes the key away from every other app.

Decisions from the design discussion (2026-11-14):
- Compositor binds stay the mechanism, so the key never reaches the game. The desktop shortcut portal (GlobalShortcuts) isn't in a released niri yet, and on niri it passes the key through to the focused window.
- niri merges `binds {}` blocks across included files, and it silently accepts the same key bound twice (both checked with `niri validate`). So setup must find conflicts itself.
- Buttons are pointer-only; the Overlay still never takes keyboard focus.

**Blocked by:** 06 (Currency Rates panel toggled by hotkey)

**Status:** ready-for-agent

- [ ] `[hotkeys]` in config.toml maps each Action to a key (default `currency-rates = "Alt+R"`); invalid keys or unknown Actions are reported like other config mistakes
- [ ] On first launch under niri with hotkeys not yet set up, the Route Panel offers setup; "Set up" installs them and "Not now" dismisses them, and both answers are remembered across restarts
- [ ] Setup writes `poe-overlay.kdl` and adds exactly one `include` line to `config.kdl` (idempotent); niri's config stays valid (`niri validate`) and the bind works without restarting niri
- [ ] A key already bound in the player's niri config (including included files) is not installed; the Panel and the CLI name the conflict and point to `[hotkeys]`
- [ ] `poe-overlay setup hotkeys` and `--remove` work from a terminal; `--remove` leaves the rest of the config byte-for-byte unchanged
- [ ] Under an unknown compositor, the Panel line and the CLI show the `poe-overlay trigger …` command to bind by hand instead of offering setup
- [ ] README explains first-launch setup, `[hotkeys]`, `setup hotkeys` and `--remove`; the manual niri snippet stays as the fallback
- [ ] The current hand-written `Alt+R` line in the user's `~/.config/niri/cfg/keybinds.kdl` (added 2026-11-14) is moved into the generated file, so it isn't bound twice
