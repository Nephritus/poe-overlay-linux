# 15: Focus tracking on KDE, Hyprland and sway

**What to build:** Showing panels only while the game is focused, and on the game's monitor, works beyond niri: KDE Plasma via its window-management interface, Hyprland and sway via foreign-toplevel. README lists supported compositors and states GNOME is unsupported (no layer-shell). Hotkeys no longer need per-compositor work: ticket 18 grabs them through Xwayland.

**Blocked by:** 02 (Companion skeleton)

**Status:** ready-for-human

- [x] On KDE Plasma 6, Hyprland and sway the panel shows only while PoE is focused, on its monitor
- [x] Unsupported compositor (no layer-shell) exits with a clear message naming the requirement

## Comments

### Implemented (agent, 2026-11-17)

- **Hyprland and sway** needed no new code: both offer wlr-foreign-toplevel-management with `output_enter`, and give Xwayland windows their X11 class as app_id (checked in Hyprland's `ForeignToplevelWlr.cpp` and sway's `xwayland.c`, then live).
- **KDE Plasma:** KWin has no wlr-foreign-toplevel, so `toplevels.rs` falls back to `org_kde_plasma_window_management` (v13+, `window_with_uuid`). Plasma windows give a position, not outputs: `focus::outputs_under` picks the output holding most of the window, from the outputs' logical areas (xdg-output). `PlasmaWindows` turns KWin's one-property-at-a-time events into the same `Toplevel` updates, reported from `initial_state` on and only when something that decides visibility changed (KWin sends the geometry of every window being dragged). Unplugged outputs are forgotten.
- **KWin 6.0–6.7 permission** (user's choice: silent, like the Wine cursor fix): those versions give the window list only to a program whose desktop entry runs its executable and lists `X-KDE-Wayland-Interfaces=org_kde_plasma_window_management`; 6.8 only restricts sandboxed apps (kwin source, tags v6.0.0–v6.8.2). Under KDE (`XDG_CURRENT_DESKTOP` contains `KDE`) the overlay writes a hidden `io.github.nephritus.PoeOverlay.KWin.desktop` into `$XDG_DATA_HOME/applications`, rewritten only when the executable's path changes (AppImage). Right after writing it, a missing window list is asked for once more after 2 s (KService rechecks its cache at most that often). README documents the file.
- **No layer-shell / no Wayland:** the overlay prints that it needs wlr-layer-shell (GNOME has none) and exits; GDK is limited to Wayland, so without a Wayland session it says that instead of opening a plain X11 window. The game still starts (`launch` spawns the overlay detached).
- **Tests:** `tests/game_focus.rs` (outputs under a window: offset and straddling monitors, off every output), `tests/plasma_windows.rs` (5: reported at initial state, later changes, drags reporting nothing, unmapped, focus game → Firefox → game), `tests/kwin_permission.rs` (4: contents, rewritten only when moved, quoted path with spaces, other desktops). Every test was checked to fail when the code it covers is broken. The launch tests now also keep `XDG_DATA_HOME` temporary.
- **Checked live,** each compositor nested in niri with a stand-in X11 game (a terminal titled "Path of Exile"):
  - **sway 1.12** (two outputs): the Route Panel showed on the game's output and followed it when moved to the other; hid when a Wayland window was focused.
  - **Hyprland 0.57.2** (two outputs): the same.
  - **KWin 6.8.3:** its nested Wayland backend fails at `eglInitialize` on this hybrid AMD/NVIDIA laptop, so it ran headless (`--virtual`, two outputs) and was observed through KWin scripting. The overlay's layer surface sat on the game's output, moved with it both ways, disappeared while a Wayland window was active and came back with the game. The hidden entry was written; 6.8 doesn't need it, so the 6.0–6.7 path (and the 2 s retry) is untested.
  - **Weston 15** (no layer-shell): the message, and the game ran on.
- **After review:** the Plasma path no longer reports every drag; outputs removed from the registry are dropped; docs fixed (outputs order, one name for the "entry just written" flag). Not changed: `Exec` escaping beyond quoting a path with spaces, since KWin compares the first word of the command with the executable's path, and a path with `"` or `\` is unlikely for a game tool.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| KDE Plasma 6.0–6.7 | On a real Plasma session before 6.8 (e.g. Kubuntu, Debian), the Route Panel shows with the game focused on the first launch (the entry is written then) | |
| KDE Plasma, real game | The panel shows only with PoE focused, on its monitor | |
| GNOME | Launching the game prints the layer-shell message and the game runs | |
