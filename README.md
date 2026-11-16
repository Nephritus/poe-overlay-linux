# poe-overlay

An overlay for Path of Exile 1 on Wayland, for trading and campaign leveling. Early development.

This product isn't affiliated with or endorsed by Grinding Gear Games in any way.

## Requirements

- A Wayland compositor with `wlr-layer-shell`, `wlr-foreign-toplevel-management` and Xwayland. niri is tested so far. GNOME is not supported.
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

While the game is focused, the overlay catches its hotkeys itself and carries out their Actions. The game doesn't receive those keys; other windows keep them, so Alt+D still works in your browser. Nothing is added to your compositor's config. Actions and their default keys:

- `currency-rates` (Alt+R): show or hide the Currency Rates Panel.
- `price-check` (Alt+D): price the item under the pointer (see [Price Check](#price-check)).
- `settings` (no key): open the [Settings](#settings) window.

Choose keys in `~/.config/poe-overlay/config.toml`. Modifiers are Ctrl, Shift, Alt and Super; keys are XKB key names such as `R`, `F5` or `Return`. `""` leaves an Action without a hotkey:

```toml
[hotkeys]
currency-rates = "Ctrl+F7"
```

Changes apply the next time the game starts. An Action runs when you let go of its key, not when you press it. Holding the key runs it once.

How it works: under Proton the game is an X11 window, so its keys pass through Xwayland. While the game has focus, the overlay grabs the hotkeys' keys there (an X11 key grab), and it lets them go when the game loses focus. This works on any compositor with Xwayland. A key your compositor binds never reaches Xwayland, so a compositor bind on the same key wins; pick another key if a hotkey does nothing. The Route Panel says so if a key isn't on your keyboard layout or another X11 program has grabbed it.

### Without Xwayland

If the game runs as a native Wayland window (e.g. `PROTON_ENABLE_WAYLAND=1`), its keys never pass through Xwayland and the hotkeys do nothing. Bind keys in your compositor to `poe-overlay trigger <action>` instead, using the program's full path or a place on the session's PATH (`cargo install --path .` puts it in `~/.cargo/bin`). For niri:

```kdl
Alt+R { spawn "poe-overlay" "trigger" "currency-rates"; }
```

If no Overlay is running, the trigger exits with an error saying so.

## Currency Rates

The Currency Rates Panel lists the 12 most traded currencies of the league, priced in chaos and divine, from [poe.ninja](https://poe.ninja). Prices are fetched when the Panel opens and reused for 15 minutes. If poe.ninja can't be reached, the Panel says why and tries again a minute later at the earliest. The Panel closes when you switch away from the game. The league is the one poe.ninja lists first, normally the current challenge league; pick another in the [Settings](#settings) window.

## Settings

`poe-overlay settings` opens a normal window, whether or not the game runs. It's the one place you type into; the overlay itself never takes keyboard focus. It sets:

- **League**: the league Currency Rates and Price Check price in, from the list poe.ninja prices. "Current challenge league" follows poe.ninja's first league from one league to the next.
- **Client.txt**: where the game's log is, for installs Steam doesn't know (Lutris, Heroic). Leave it empty to find it through Steam. **Choose…** opens a file picker; a path that can't be read is reported in the window and not saved.

**Save** writes them to `~/.config/poe-overlay/config.toml`, keeping your comments and other settings:

```toml
[prices]
league = "Hardcore Covens"

[game]
client_txt = "/home/you/Games/path-of-exile/drive_c/Program Files (x86)/Grinding Gear Games/Path of Exile/logs/Client.txt"
```

A running overlay picks up a new league or Client.txt within a second, also when you edit the file by hand. With a new Client.txt, the Route takes the Character from that log's latest level-up. Route and hotkey settings still apply the next time the game starts.

The `settings` Action opens the window from the game; it has no key unless you give it one under `[hotkeys]` (e.g. `settings = "Alt+F10"`).

## Price Check

Hover over an item in game and press the Price Check hotkey (Alt+D). The overlay presses Ctrl+Alt+C for you, reads the item text the game copies, and shows its [poe.ninja](https://poe.ninja) price in chaos and divine, on the half of the screen away from the pointer:

- **Uniques** by name, at their links (5 and 6 links are priced on their own) and with each variant poe.ninja lists. An unidentified unique lists the uniques of its base.
- **Gems** at their level, quality and corruption; when poe.ninja has no such entry, the closest ones are shown, labelled. Imbued gems are priced with their support.
- **Currency** and other stackables (oils, essences, fossils, fragments, scarabs…), each and for the stack.
- **Divination cards**, each and for the stack.

Pressing the hotkey again closes the Panel when the pointer is on the same item or on nothing; on another item, it prices that one. The ✕ button and switching away from the game close it too. The game must be the English client.

### Rare, magic and normal items

poe.ninja doesn't price these, so the overlay searches the [official trade site](https://www.pathofexile.com/trade) for items like it, in the league prices come from. The Panel opens wide: the search on the left, the 10 cheapest online listings on the right (price, how long ago it was listed, and AFK sellers). The search asks for online sellers, the same base type, and at least 90% of the item's:

- total life (flat life plus half the strength, as the trade site counts it), total elemental resistance, chaos resistance, strength, dexterity and intelligence, counting every modifier;
- other explicit (and fractured) modifiers, each on its own. Implicit and crafted modifiers only count toward the totals.

Each stat the search uses is a row on the left with its minimum. Untick the ones that don't matter and press **Search** to see what that finds; ticking alone sends nothing, and Search waits while a search is out. Modifiers left out of the search are listed under it: ones the trade site has no stat for, and implicit, crafted and enchant modifiers that aren't in a total. The stats and base types come from a snapshot of the trade site's data in `data/trade`, refreshed with `data/trade/update.sh` when a league adds new ones.

**About the trade site:** its search endpoints aren't part of GGG's documented API, and GGG's [developer docs](https://www.pathofexile.com/developer/docs) say unlisted endpoints aren't supported. Every Price Check of an item poe.ninja doesn't price, and every press of Search, sends one search and one fetch, the way Awakened PoE Trade and Sidekick do, with a browser User-Agent (the site refuses others). The overlay follows the rate limits the site states in its answers and sends nothing while one is reached; the Panel says when you can search again. After a refusal (HTTP 403) it waits five minutes. The same search within five minutes reuses the last answer.

The clipboard is read once per press (GGG's rule for tools). Before the copy, the overlay puts a placeholder on the clipboard: with Proton 11 the game's copy otherwise doesn't reach other programs. The copied item stays on your clipboard, as when you copy it yourself.

Keys are pressed through a virtual keyboard (`/dev/uinput`), which works on any compositor. Steam's udev rules already allow it on most systems; if not, the Panel shows the rule to add:

```
KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"
```

Put it in `/etc/udev/rules.d/60-poe-overlay-uinput.rules`, then run `sudo udevadm control --reload && sudo udevadm trigger`.

## Leveling Route

While the game is focused, a Route panel shows the Steps for the Area your Character is in, acts 1 to 10. It moves to the right Steps when the game's `Client.txt` reports a new Area. `Client.txt` is found through Steam's library folders; for other installs (Lutris, Heroic), choose it in the [Settings](#settings) window.

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

Leveling Route data comes from [exile-leveling](https://github.com/HeartofPhos/exile-leveling) by HeartofPhos, used under the MIT License (see `data/exile-leveling/LICENSE`). Trade stats and base types come from the trade site (`data/trade`). Game names and text belong to Grinding Gear Games. `poe-overlay credits` prints this notice and the license.
