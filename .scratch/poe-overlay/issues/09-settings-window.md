# 09: Settings window

**What to build:** `poe-overlay settings` (also bindable to a key) opens a normal application window, separate from the overlay, where the user can type and choose settings: league and a Client.txt path override for non-Steam installs (Lutris, Heroic). Settings are saved to the config the overlay reads and take effect without restarting the game. This window is the only place with text input; the overlay itself still never takes keyboard focus.

**Blocked by:** 03 (Route follows Area changes), 06 (Currency Rates panel toggled by hotkey)

**Status:** ready-for-human

- [x] Command opens the window whether or not the overlay is running
- [x] Changing the league updates Currency Rates and Price Check without restarting
- [x] A Client.txt path override is used instead of Steam detection
- [x] Invalid path is reported in the window

## Comments

### Implemented (agent, 2026-11-16)

- **Window** (`src/settings_window.rs`): `poe-overlay settings`, a normal GTK window in its own process. A second start shows the open window.
  - **League:** a dropdown with "Current challenge league (…)" plus poe.ninja's league list, fetched off the GTK thread. A configured league poe.ninja doesn't list stays selectable.
  - **Client.txt:** a path field with **Choose…** (GTK file dialog); the hint shows which file Steam detection finds.
  - **Save** checks the path first (a relative path, missing file, folder or unreadable file each get their own message, shown in red, nothing saved), then rewrites `config.toml` as it is on disk at that moment.
  - A `config.toml` with a TOML syntax error is shown and Save is disabled, so the player's file is never overwritten.
- **File editing** (`config::ConfigFile`, `toml_edit`): comments, other sections and unknown keys stay byte for byte; clearing a setting removes the key and a section left empty. The new key is `[game] client_txt`.
- **Live in the overlay:** `config.toml` is checked once a second (also catches hand edits).
  - A new league switches `PriceSource` (`set_league`; caches per league stay) and reloads an open Currency Rates Panel.
  - A new Client.txt replaces the followed log (the old reader thread ends) and the Route takes the Character from its latest level-up, like a level-up.
  - The "Client.txt not found" and "settings file has a mistake" notes now clear when fixed (Route Panel notes can be replaced).
  - Route options and hotkeys still apply at the next start (README says so).
- **Action `settings`** with no default key (`settings = "…"` under `[hotkeys]`); the overlay starts `poe-overlay settings`. `trigger settings` works too.
- **Found on the way:** with permanent Hardcore chosen, poe.ninja's currency overview has two lines, a chaos line without `primaryValue` and no divine rate; the Panel showed "unexpected currency overview (missing field `primaryValue`)". It now says "poe.ninja has too few trades in Hardcore to price currency." (recorded response in `tests/data/ninja-exchange-currency-hardcore.json`).
- **Tests:** `tests/config_file.rs` (7), Client.txt checks and override in `tests/client_log.rs` / `tests/steam_install.rs`, league change and league list in `tests/prices.rs` / `tests/item_prices.rs`. Each new test was checked to fail when the code it covers is broken.
- **Checked live** on niri with a stand-in game and temporary config/state folders. The window was driven through AT-SPI and keys sent to it only while it had focus:
  - `trigger settings` opened the window over the game.
  - `/nonexistent/Client.txt` + Save: "No file at /nonexistent/Client.txt." in red, "Not saved", file unchanged.
  - League Standard + a temporary Client.txt + Save: `config.toml` kept the player's comment and `[route]`, gained `[prices]` and `[game]`.
  - Back in the game: the Route showed the new log's Character (SmokeTester), and an Area line appended to that log moved it to The Coast. Currency Rates opened as "Currency Rates · Standard".
  - Hand edit to `Hardcore` while the Panel was open: it reloaded within seconds (and showed the poe.ninja problem fixed above).
- **After review:**
  - The window also refuses a `config.toml` the overlay itself would reject (e.g. an unknown key), so "Saved" always means a running overlay takes it.
  - A Client.txt that can't be read (e.g. a hand-edited path) is named on the Route Panel; it's still followed, in case the game creates it later.
  - Switching Client.txt reads Steam's library list and the log's latest level-up off the GTK thread (logs reach hundreds of MB), and lines still queued from the old log are dropped.
  - `config.toml` is watched from before it's first loaded, so a save in between isn't missed.
  - Re-checked live: a missing chosen path showed "No file at … The Route follows your Areas once it can read it."; a hand edit to a real log switched the Route to its Character and followed its Area line; a typo'd key showed the mistake on the Route Panel and in the window, with Save disabled.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Window | `poe-overlay settings` opens it; the dropdown lists poe.ninja's leagues; **Choose…** opens your file picker | |
| League in game | Pick Standard, Save, back in PoE: Alt+R and Alt+D price in Standard | |
| Lutris/Heroic | With a non-Steam install, choosing its Client.txt makes the Route follow your Areas without restarting the game | |
