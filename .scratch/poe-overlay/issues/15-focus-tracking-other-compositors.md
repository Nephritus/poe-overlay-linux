# 15: Focus tracking and hotkey setup on KDE, Hyprland and sway

**What to build:** Showing panels only while the game is focused, and on the game's monitor, works beyond niri: KDE Plasma via its window-management interface, Hyprland and sway via foreign-toplevel. README lists supported compositors, states GNOME is unsupported, and includes hotkey bind snippets for each.

**Blocked by:** 02 (Companion skeleton), 17 (Hotkeys set up by the overlay on first launch)

**Status:** ready-for-agent

- [ ] On KDE Plasma 6, Hyprland and sway the panel shows only while PoE is focused, on its monitor
- [ ] Unsupported compositor (no layer-shell) exits with a clear message naming the requirement
- [ ] README has bind snippets for niri, Hyprland, sway and KDE
- [ ] Hotkey setup (ticket 17) works on Hyprland: generated `poe-overlay.conf`, one `source =` line in `hyprland.conf`, `hyprctl reload`; conflicts found via `hyprctl binds`
- [ ] Hotkey setup works on sway: generated file, one `include` line, `swaymsg reload`; conflicts found via `swaymsg -t get_config`
- [ ] Hotkey setup works on KDE Plasma through the desktop shortcut portal (GlobalShortcuts): the overlay registers its Actions and Plasma's own dialog confirms the keys; no config file is edited. Record whether the key still reaches the game
- [ ] COSMIC: find out whether custom-shortcut files or the portal work, and implement whichever does, or fall back to showing the command (unverified as of 2026-11-14)
- [ ] Compositor detection uses the session environment (`NIRI_SOCKET`, `HYPRLAND_INSTANCE_SIGNATURE`, `SWAYSOCK`, KDE session variables)
