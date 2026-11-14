# poe-overlay

An overlay for Path of Exile 1 on Wayland, for trading and campaign leveling. Early development.

This product isn't affiliated with or endorsed by Grinding Gear Games in any way.

## Requirements

- A Wayland compositor with `wlr-layer-shell` and `wlr-foreign-toplevel-management`. niri is tested so far. GNOME is not supported.
- GTK 4 and gtk4-layer-shell. On Arch: `pacman -S gtk4 gtk4-layer-shell`.
- The English game client.

## Build

```sh
cargo build --release
```

## Use with Steam

Set the game's launch options to:

```
/path/to/poe-overlay launch -- %command%
```

The overlay starts with the game, shows only while the game window is focused (on the game's monitor), and exits when the game exits. It never takes keyboard focus. Only one overlay runs at a time.

### Cursor lock fix

On a single-monitor desktop, Wine locks the cursor to a full-screen game window until focus moves away. Under Wayland that keeps the cursor out of the overlay's panels. Before each start, `launch` sets `"GrabPointer"="N"` for the PoE executables in the game's Wine prefix (`HKCU\Software\Wine\AppDefaults\<exe>\X11 Driver` in `user.reg`). It saves a one-time copy as `user.reg.poe-overlay-backup`. PoE's own "Confine mouse to window" option then has no effect.

To opt out, set `"GrabPointer"="Y"` in that section. `launch` never changes a value that's already there.

## Hotkeys

Hotkeys are keybinds in your compositor that run `poe-overlay trigger <action>`. The running Overlay carries out the action while the game is focused. Actions and their default keys:

- `currency-rates` (Alt+R): show or hide the Currency Rates Panel.

Choose keys in `~/.config/poe-overlay/config.toml`. Modifiers are Ctrl, Shift, Alt and Super; keys are XKB key names such as `R`, `F5` or `Return`. `""` leaves an Action without a hotkey:

```toml
[hotkeys]
currency-rates = "Ctrl+F7"
```

### niri: set up for you

The first time the game starts under niri, the Route Panel asks whether to set the hotkeys up. **Set up** writes them to `~/.config/niri/poe-overlay.kdl` and adds one `include` line at the end of `config.kdl`; niri picks them up at once and keeps the key from reaching the game. **Not now** is remembered. After that, keys changed in `config.toml` are applied the next time the game starts.

A key you already use in your niri config is left to your bind (niri would otherwise let ours win silently); the Panel says where it's bound so you can pick another. An Action you bound to `poe-overlay trigger` yourself is left alone.

From a terminal:

- `poe-overlay setup hotkeys`: set them up now (also after "Not now").
- `poe-overlay setup hotkeys --remove`: delete `poe-overlay.kdl` and the `include` line, and stop the game asking.

To bind them yourself instead, add lines like this inside `binds { }` in your niri config; poe-overlay then leaves that Action alone:

```kdl
Alt+R { spawn "poe-overlay" "trigger" "currency-rates"; }
```

### Other compositors

Bind each key to the trigger command yourself; the Route Panel and `poe-overlay setup hotkeys` show the commands. The compositor must find `poe-overlay`: use its full path, or install it where the session's PATH looks (`cargo install --path .` puts it in `~/.cargo/bin`).

If no Overlay is running, the trigger exits with an error saying so.

## Currency Rates

The Currency Rates Panel lists the 12 most traded currencies of the league, priced in chaos and divine, from [poe.ninja](https://poe.ninja). Prices are fetched when the Panel opens and reused for 15 minutes. If poe.ninja can't be reached, the Panel says why and tries again a minute later at the earliest. The Panel closes when you switch away from the game. The league is the one poe.ninja lists first, normally the current challenge league; to pick another:

```toml
[prices]
league = "Hardcore Covens"
```

## Leveling Route

While the game is focused, a Route panel shows the Steps for the Area your Character is in, acts 1 to 10. It moves to the right Steps when the game's `Client.txt` reports a new Area. `Client.txt` is found through Steam's library folders.

The Route variant is set in `~/.config/poe-overlay/config.toml` (or under `$XDG_CONFIG_HOME`). The defaults are exile-leveling's:

```toml
[route]
league_start = true   # side trips that pay off on a fresh league
bandit = "alira"      # kill_all, oak, kraityn or alira
library = true        # include the Library in Act 3
```

Changes apply the next time the game starts. If the file has a mistake, the overlay reports it and uses the defaults.

### Progress per Character

The ◀ and ▶ buttons on the panel move to the previous or next Area visit when the Route guesses wrong. Clicking them doesn't take focus from the game.

Progress is saved per Character in `~/.local/state/poe-overlay/progress.toml` (or under `$XDG_STATE_HOME`) and restored the next time the game starts. `Client.txt` names your Character only when it levels up, so:

- At startup, the overlay assumes the Character from the latest level-up already in `Client.txt`.
- A level-up of a different Character switches to that Character's saved progress. The previous Character keeps its position from its own last level-up, because Areas entered in between may belong to either.
- A brand-new Character takes over the progress made so far in the session once it first levels up.
- In a party, another player's level-up may switch Characters. If that happens, use ◀ ▶ to correct it.

## Credits

Leveling Route data comes from [exile-leveling](https://github.com/HeartofPhos/exile-leveling) by HeartofPhos, used under the MIT License (see `data/exile-leveling/LICENSE`). Game names and text belong to Grinding Gear Games. `poe-overlay credits` prints this notice and the license.
