# poe-overlay

An overlay for Path of Exile 1 on Wayland, for trading and campaign leveling. Early development.

This product isn't affiliated with or endorsed by Grinding Gear Games in any way.

## Requirements

- A Wayland compositor with `wlr-layer-shell` (the overlay draws over the game with it), a list of windows (`wlr-foreign-toplevel-management`, or KDE Plasma's window management) and Xwayland: niri, KDE Plasma 6, Hyprland or sway. GNOME has no layer-shell and isn't supported: the overlay prints why and exits, and the game runs without it.
- On KDE Plasma 6.0 to 6.7, KWin shows its window list only to programs whose desktop entry asks for it. The overlay writes a hidden one for itself, `~/.local/share/applications/io.github.nephritus.PoeOverlay.KWin.desktop`, and rewrites it when the program moves (an AppImage runs from a new place each time). Plasma 6.8 and later don't need it. Delete it if you remove the overlay.
- GTK 4.12+ and gtk4-layer-shell 1.0+, which the packages pull in and the AppImage bundles. Ubuntu 24.04 has no gtk4-layer-shell: use the AppImage there.
- The English game client.

## Install

Each [release](https://github.com/nephritus/poe-overlay-linux/releases) has these, all installing the program as `poe-overlay`. Flatpak isn't offered: its sandbox keeps the overlay from `/dev/uinput`, the Steam folder and the game's process.

| Distro | Install |
|---|---|
| Arch, Manjaro, CachyOS, EndeavourOS | `poe-overlay` from the AUR, e.g. `paru -S poe-overlay` |
| Debian 13+, Ubuntu 25.04+ | `sudo apt install ./poe-overlay_*_amd64.deb` |
| Fedora 43+ | `sudo dnf install ./poe-overlay-*.fc43.x86_64.rpm` |
| Any other (Ubuntu 24.04, SteamOS, …) | the `.AppImage`: make it executable and put it where it stays, e.g. `~/Applications/poe-overlay.AppImage` |
| With Rust | `cargo install --locked --git https://github.com/nephritus/poe-overlay-linux`, after installing GTK 4 and gtk4-layer-shell with their development files (e.g. `libgtk-4-dev libgtk4-layer-shell-dev` on Debian) |

The packages also install the [uinput rule](#price-check-keys) and a "PoE Overlay Settings" menu entry. The AppImage carries every library it needs and starts the game with the environment Steam gave it, not with the bundled libraries' settings.

### Building the releases

`packaging/release.sh` builds the source tarball, the .deb (in Debian 13), the .rpm (in Fedora 43) and the AppImage (in Arch Linux, with [quick-sharun](https://github.com/pkgforge-dev/Anylinux-AppImages)) into `dist/`, each in a podman container, and fills in the AUR package (`dist/aur/PKGBUILD`, `.SRCINFO`) with the tarball's checksum. It then installs each artifact, the AUR package and `cargo install` into clean containers and checks that each runs, starts its overlay under a headless compositor, keeps one overlay when launched twice, and stops it when the game exits. The container images (by digest), the Rust version and the AppImage tools are pinned and file times come from the last commit, but the distro packages are whatever the mirrors have that day, so a rebuild matches in content, not byte for byte. A version tag runs the same command on GitHub Actions (`.github/workflows/release.yml`) and attaches the artifacts to the release; then push `dist/aur/PKGBUILD` and `.SRCINFO` to the AUR's `poe-overlay` repository.

For development, `cargo build --release`.

## Use with Steam

Set the game's launch options to:

```
poe-overlay launch -- %command%
```

With the AppImage or a program outside Steam's PATH, use its full path, e.g. `~/Applications/poe-overlay.AppImage launch -- %command%`.

The overlay starts with the game, shows only while the game window is focused (on the game's monitor), and exits when the game exits. It takes keys from the game only while you type in one of its boxes (a Price Check minimum or maximum). Only one overlay runs at a time.

To uncover something in the game, drag a Panel by its border: the pointer turns into a move cursor there. The game keeps the keyboard. A Panel stays wholly on screen and stays where you put it until the game restarts; Price Check then no longer moves to the half away from the pointer, and the Trade Request cards no longer re-centre.

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

`poe-overlay settings` opens a normal window, whether or not the game runs. Apart from the Price Check minimum and maximum boxes, it's the one place you type into. It sets:

- **League**: the league Currency Rates and Price Check price in, from the list poe.ninja prices. "Current challenge league" follows poe.ninja's first league from one league to the next.
- **Client.txt**: where the game's log is, for installs Steam doesn't know (Lutris, Heroic). Leave it empty to find it through Steam. **Choose…** opens a file picker; a path that can't be read is reported in the window and not saved.
- **Trade site**: **Log in** and **Log out**, below; these take effect at once, without Save.

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

poe.ninja doesn't price these, so the overlay searches the [official trade site](https://www.pathofexile.com/trade) for items like it, in the league prices come from. The Panel opens wide: the search on the left, the 10 cheapest listings on the right (price, how long ago it was listed, and whether it's an instant buyout from a Merchant's Tab, in person from a seller online now, or in person from an AFK seller). The first search asks for both kinds of sale, the same base type, and at least 90% of the item's:

- total life (flat life plus half the strength, as the trade site counts it), total elemental resistance, chaos resistance, strength, dexterity and intelligence, counting every modifier;
- other explicit (and fractured) modifiers, each on its own. Implicit and crafted modifiers only count toward the totals.

Each stat the search uses is a row on the left with its minimum and maximum, under three choices: **Item** (the item's base, or any item of its class such as "Any Gloves"), **Rarity** (any non-unique, rare, magic, normal) and **Sale** (both, instant buyout, in person). Untick the stats that don't matter, pick other choices, or click a box and type another bound (empty means none), then press **Search** or Enter to see what that finds; ticking or picking alone sends nothing, and Search waits while a search is out. While you type, the game gets no keys: the Panel says so, and Enter, Esc or closing the Panel gives them back. Modifiers left out of the search are listed under it: ones the trade site has no stat for, and implicit, crafted and enchant modifiers that aren't in a total. The stats and base types come from a snapshot of the trade site's data in `data/trade`, refreshed with `data/trade/update.sh` when a league adds new ones.

Each Listing has a button to reach its seller; nothing is sent without a click, and each click sends one thing:

- **Whisper** (in person): sends the trade site's whisper for that Listing in game chat, pasted like a [Trade Request](#trade-requests) button's line (your clipboard is put back). The site words it in the seller's language, so a whisper in German or Russian makes no outgoing Trade Request card.
- **Travel to Hideout** (instant buyout): asks the trade site to send you to the seller's hideout, the same request as the site's own button; the game teleports you. It needs your [trade site login](#logging-in-to-the-trade-site): without it the button says **Log in** and opens the Settings window; once the overlay has your login, Search fetches the Listings again with it. If others are buying the item too, the row says so and the next click goes anyway, as on the site. The way to a hideout expires five minutes after the search; an older Listing asks you to press Search instead of sending anything. A refusal shows on the row with the site's reason.

**About the trade site:** its search endpoints aren't part of GGG's documented API, and GGG's [developer docs](https://www.pathofexile.com/developer/docs) say unlisted endpoints aren't supported. Every Price Check of an item poe.ninja doesn't price, and every press of Search, sends one search and one fetch, the way Awakened PoE Trade and Sidekick do, with a browser User-Agent (the site refuses others). The overlay follows the rate limits the site states in its answers and sends nothing while one is reached; the Panel says when you can search again. After a refusal (HTTP 403) it waits five minutes. The same search within five minutes reuses the last answer.

The clipboard is read once per press (GGG's rule for tools). Before the copy, the overlay puts a placeholder on the clipboard: with Proton 11 the game's copy otherwise doesn't reach other programs. The copied item stays on your clipboard, as when you copy it yourself.

<a id="price-check-keys"></a>Keys are pressed through a virtual keyboard (`/dev/uinput`), which works on any compositor. The packages install a udev rule allowing it, and Steam's udev rules already do on most systems; if not (AppImage, `cargo install`), the Panel shows the rule to add:

```
KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"
```

Put it in `/etc/udev/rules.d/60-poe-overlay-uinput.rules`, then run `sudo udevadm control --reload && sudo udevadm trigger`.

### Logging in to the trade site

Searches work without logging in. Logged in, the site counts them against your account as well as your address. **Log in** in the Settings window looks for a login your browser already holds; if there is none, it opens your default browser on the pathofexile.com login page, and the window says "Logged in with Firefox" (or another browser) once you've logged in there. There is nothing to copy.

The overlay reads the login from the browser's cookie file at the first search, again when one of its cookies expires, and at most once a minute while there's no login or no trade token yet. It copies the file (and its write-ahead log, so an open browser's latest login counts) into a private folder in `$XDG_RUNTIME_DIR`, reads only the cookies of pathofexile.com and its subdomains, and deletes the copy at once. Of those it keeps, in memory only, the session (`POESESSID`), the trade token (`POETOKEN`) and Cloudflare's clearance (`cf_clearance`); they're never written anywhere or logged. Searches send them with the User-Agent of the browser they came from, since the clearance holds only for that browser. The trade token lasts a day and the site doesn't renew it in its answers, so once it expires the overlay reads the browser again: opening the trade site in the browser gets a new one.

Read from these folders under your home folder, the default browser's first; with logins in several, the default browser's is used, else the most recent:

| Browser | Profiles |
|---|---|
| Firefox | `.config/mozilla/firefox` (Firefox 147+), `.mozilla/firefox`, Snap, Flatpak `org.mozilla.firefox` |
| LibreWolf | `.config/librewolf/librewolf`, `.librewolf`, Flatpak `io.gitlab.librewolf-community` |
| Zen | `.config/zen`, `.zen`, Flatpak `app.zen_browser.zen` |
| Floorp | `.floorp`, Flatpak `one.ablaze.floorp` |
| Waterfox | `.waterfox`, Flatpak `net.waterfox.waterfox` |
| Chrome | `.config/google-chrome`, Flatpak `com.google.Chrome` |
| Chromium | `.config/chromium`, Snap, Flatpak `org.chromium.Chromium` |
| Brave | `.config/BraveSoftware/Brave-Browser`, Flatpak `com.brave.Browser` |
| Edge | `.config/microsoft-edge`, Flatpak `com.microsoft.Edge` |
| Vivaldi | `.config/vivaldi`, Flatpak `com.vivaldi.Vivaldi` |
| Opera | `.config/opera`, Flatpak `com.opera.Opera` |

Chromium-family browsers encrypt their cookies. The overlay decrypts the pathofexile.com ones with the key the browser keeps in your keyring as "Chrome Safe Storage" (Chromium, Brave: their own name; Edge and Opera use Chromium's, Vivaldi Chrome's): from the Secret Service (GNOME Keyring, KDE's, KeePassXC), else from KWallet if it's open, else the fixed key Chromium uses without a keyring. It never asks to unlock a keyring: a locked keyring or a missing key shows in the Settings window as the reason no login was found. Opera's and Vivaldi's User-Agent can't be told from their files, so their logins are sent with the overlay's usual one.

**Log out** makes the overlay forget the login and stop reading the browser; the browser stays logged in. The choice is `login = true` under `[trade]` in config.toml.

## Trade Requests

Trade-site whispers in `Client.txt` appear as cards at the top centre of the screen: who, the item, the price, the stash tab and position, and anything the buyer added. Buyers' whispers are gold, your own whispers to sellers blue. Bulk currency requests ("1 Divine Orb for my 100 Chaos Orb") show without a stash spot. Cards stack newest on top, five at most; each stays until you close it with ✕. Other whispers make no card. Only English whisper templates are recognised: a buyer whose game runs in another language sends a translated whisper, which shows no card.

A card's buttons each send one chat line to the game, only when clicked:

| Card | Button | Sends |
|---|---|---|
| Buyer's | Invite / Trade / Kick | `/invite`, `/tradewith`, `/kick` with the buyer's name |
| Buyer's | Thank / Sold | `@buyer Thanks, enjoy!` / `@buyer Sorry, it's already sold.` |
| Yours | Hideout / Trade / Thank | `/hideout`, `/tradewith` with the seller's name, `@seller Thanks, enjoy!` |
| Yours | Leave | `/leave` |

The line is pasted into chat (Enter, Ctrl+V, Enter on the virtual keyboard), so any keyboard layout works. Your clipboard is saved first, every type it held (text, images), up to 16 MiB, and put back afterwards; the overlay then hands it to whatever pastes it, until you copy something else. If you were typing in a Price Check box, the keys go back to the game first.

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
