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
