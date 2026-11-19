# 04: The Menu

**What to build:** The `menu` Action (default Alt+M) opens the Menu: a Panel centred in the Overlay that can be moved like the others. It has a column of sections; this ticket ships General (league, Client.txt path box with "Choose a file…" opening the Settings window, trade login) from the shared form of 03. ✕, the `menu` Hotkey again, or leaving the game closes it. The game keeps the keyboard while the Menu is open; typing in a Menu box takes it, and Enter, Esc or closing the Menu gives it back, as with Price Check bounds. The v0.1.0 `settings` Action is renamed `menu`; a leftover `settings = …` in `[hotkeys]` shows a notice naming the new key and is otherwise ignored. GLOSSARY, README and `poe-overlay trigger menu` follow.

**Blocked by:** 03 (Settings save as you change them)

**Status:** ready-for-agent

- [ ] Alt+M opens and closes the Menu in game; ✕ and leaving the game close it
- [ ] With the Menu open and no box being typed in, game keys still reach the game (niri reports the game focused)
- [ ] Changing the league in the Menu writes config.toml and the Currency Rates Panel prices in it
- [ ] "Choose a file…" opens the Settings window
- [ ] A config.toml with `settings = "…"` under `[hotkeys]` loads, and the notice names `menu`
- [ ] `poe-overlay trigger menu` toggles the Menu
