# 15: Focus tracking on KDE, Hyprland and sway

**What to build:** Showing panels only while the game is focused, and on the game's monitor, works beyond niri: KDE Plasma via its window-management interface, Hyprland and sway via foreign-toplevel. README lists supported compositors and states GNOME is unsupported (no layer-shell). Hotkeys no longer need per-compositor work: ticket 18 grabs them through Xwayland.

**Blocked by:** 02 (Companion skeleton)

**Status:** ready-for-agent

- [ ] On KDE Plasma 6, Hyprland and sway the panel shows only while PoE is focused, on its monitor
- [ ] Unsupported compositor (no layer-shell) exits with a clear message naming the requirement
