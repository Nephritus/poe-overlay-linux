# 15: Tray icon

**What to build:** Where the desktop has a tray (StatusNotifierItem), the running Overlay shows an icon with two items: Menu (opens the Menu in game) and Quit overlay (stops the Overlay; the game keeps running). Without a tray, startup is clean with no error output. Supersedes `poe-overlay/14`.

**Blocked by:** 04 (The Menu)

**Status:** ready-for-agent

- [ ] With a tray (e.g. waybar, KDE), the icon shows while the Overlay runs and goes away when it stops
- [ ] Menu opens the Menu; Quit overlay stops the Overlay without affecting the game
- [ ] Without a tray, no error output
