# v0.2.0: the Menu, Features, and a PoE-inspired look

Vocabulary per `GLOSSARY.md` (Menu, Feature, Panel, Action, Hotkey, Settings window). Decided in a grilling session on 2026-11-19.

## Menu
- A Panel in the Overlay, opened by the `menu` Action (default Alt+M) or the tray's "Menu" item. It opens centred and can be moved like any Panel.
- It closes with ✕, the `menu` Hotkey again, or switching away from the game.
- The game keeps the keyboard while the Menu is open. The Menu takes it only while the player types in a Menu box or records a Hotkey, the same rule as Price Check bounds.
- Layout: a column of sections on the left:
  - General: league, Client.txt, trade login, UI scale, opacity, reset positions.
  - Hotkeys.
  - Route.
  - Price Check: trade-site search, number of Listings.
  - Trade Requests: Thank and Sold texts.
  - Currency Rates: which currencies.
  - Each Feature's section starts with its on/off switch.
- Client.txt in the Menu is a path box. "Choose a file…" opens the Settings window, because a file picker would take focus from the game and close the Menu.
- The `settings` Action of v0.1.0 is renamed `menu`. A leftover `settings = …` in `[hotkeys]` shows a notice naming the new key and is otherwise ignored. `poe-overlay settings` still opens the Settings window.

## Settings window
- Same settings as the Menu, through one shared form, for when the game isn't running.

## Saving and applying
- Each change is written to config.toml at once; neither the Menu nor the Settings window has a Save button.
- The running Overlay applies every setting live, Hotkeys and Route options included. Hand edits of config.toml still apply within a second, and comments survive.

## Features
- The five Features: Route, Currency Rates, Price Check, trade-site search, Trade Requests.
- A Feature that's off shows nothing and has no Hotkey: its keys go back to the game.
- While off:
  - The Route still follows Areas, so it's in the right place when turned back on.
  - Trade Requests drops whispers for good: no card, now or later.
- With Price Check on and the trade-site search off, checking a rare says "Trade-site search is off" and links to the Menu's Price Check section.

## Newly editable settings
- **Hotkeys:** click the box and press the combination; it's saved.
  - A combination another Action uses is refused, naming that Action.
  - Esc cancels; Backspace clears.
- **Route options** (league start, bandit, library): the Route rebuilds and stays in the current Area.
- **Route position:** choose an Act, then click one of its Legs, for the current Character.
- **Currency Rates:** pick and order currencies from poe.ninja's list; the default is today's 12 most traded.
- **Listings shown:** 1 to 10. The trade site returns 10 per fetch, so this never adds a request.
- **Thank and Sold:** the text after `@buyer` is editable; an empty text hides that button.

## UI/UX
- Fix the three known UI bugs (ticket 16).
- Panel positions are kept across game restarts; "Reset positions" puts them back.
- One UI scale and one background opacity for all Panels.
- A ▾ button collapses the Route Panel to the current Step; the choice is remembered.
- Tray icon (where the desktop has a tray): Menu, Quit overlay. It supersedes `poe-overlay/14`.

## Look
- PoE-inspired: dark brown/black, gold trim, serif type, our own colours and borders, no GGG art.
- The same theme everywhere: every Panel, the Menu, the Settings window.
- Fonts (OFL, bundled): Cinzel for headings, Alegreya for text. Fontin, the game's font, needs exljbris's paid extended license to ship with an app (https://exljbris.com/extended_license.html), so it's not used.
- A throwaway GTK prototype with the real GTK CSS is approved before the theme is built (ticket 01).

## Version
- v0.2.0.
