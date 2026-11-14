# 09: Settings window

**What to build:** `poe-overlay settings` (also bindable to a key) opens a normal application window, separate from the overlay, where the user can type and choose settings: league and a Client.txt path override for non-Steam installs (Lutris, Heroic). Settings are saved to the config the overlay reads and take effect without restarting the game. This window is the only place with text input; the overlay itself still never takes keyboard focus.

**Blocked by:** 03 (Route follows Area changes), 06 (Currency Rates panel toggled by hotkey)

**Status:** ready-for-agent

- [ ] Command opens the window whether or not the overlay is running
- [ ] Changing the league updates Currency Rates and Price Check without restarting
- [ ] A Client.txt path override is used instead of Steam detection
- [ ] Invalid path is reported in the window
