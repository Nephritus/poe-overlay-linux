# 07: Price Check for poe.ninja-priced items

**What to build:** The Price Check hotkey makes the overlay release any modifiers the user is still holding, send Ctrl+Alt+C to the game via a uinput virtual keyboard, and read the clipboard once through the data-control protocol. The item text is parsed; uniques, currency, gems and divination cards show their poe.ninja price. The panel opens on the screen half opposite the cursor (cursor position queried from the game's Xwayland server; fall back to the left half if unavailable) and closes on its ✕ button, pressing the hotkey again, a new Price Check, or the game losing focus. Non-English item text shows a clear "English client only" error. If /dev/uinput isn't accessible, the overlay shows the udev rule to install instead of failing silently. One clipboard read per key press (GGG's tool rule).

**Blocked by:** 06 (Currency Rates panel toggled by hotkey)

**Status:** ready-for-human

- [x] Pressing the bound hotkey over a unique, currency item, gem or divination card shows its poe.ninja price
- [ ] Holding the hotkey's modifiers does not corrupt the copy (verified with a Ctrl-based binding)
- [x] Panel appears on the half opposite the cursor and closes by each of the four rules
- [x] Exactly one clipboard read per press
- [x] Non-English item text and missing uinput access each produce their specific message
- [x] Item text parsing is covered by tests using real copied item samples

## Comments

### Implemented (agent, 2026-11-15)

- **Hotkey:** Action `price-check`, default Alt+D in `[hotkeys]`. Once hotkeys are set up in niri, the next game start adds it to `poe-overlay.kdl` by itself.
- **The copy:** a uinput virtual keyboard, created when the overlay starts, presses Ctrl+Alt+C (user's choice over plain Ctrl+C). The "c" key is looked up in the player's keyboard layout through GDK, so AZERTY and Dvorak work. Before the keys, the clipboard gets a placeholder through ext-data-control: on Proton 11 (Experimental) the game's copy otherwise doesn't reach other programs (awakened-poe-trade #1846). The placeholder carries a marker type, so it is skipped without being read, even when a clipboard manager re-offers it. Then the first new selection is read once, within 1.5 s.
- **Held modifiers:** a virtual keyboard can't release a key the physical keyboard holds; the kernel drops a release for a key that isn't down on that device. The hotkey's Shift and Super are tapped (pressed and released) before the copy; Ctrl and Alt are part of Ctrl+Alt+C anyway. This needs the in-game check below with a Ctrl-based binding.
- **Placement:** the pointer is read from the game's X server (Xwayland) and measured against the window under it, so a second monitor doesn't skew it. The Panel opens on the right half when the pointer is on the left half; otherwise, or without X (a native Wayland game), it opens next to the Route Panel. Only one Toggled Panel shows at a time: Price Check and Currency Rates take turns in that spot.
- **Closing:** the ✕ button; switching away from the game; and a press while the Panel is open, which closes it at once. The copy then reopens it only for a different item: not for the same item, and not when nothing is hovered (the user chose this over "always close").
- **Prices:** uniques by name and links (5L and 6L priced on their own; every variant listed; foil entries ignored). Unidentified uniques list the uniques on their base. Gems match level, quality and corruption exactly, or show the 3 closest entries, labelled. Imbued gems are priced via ImbuedGem by their support. Currency and other stackables are looked up in the exchange category the name suggests first (oils, scarabs, essences…), then the others. Cards come from the DivinationCard exchange. Each poe.ninja category is fetched at most once per league per 15 minutes and shared with Currency Rates. Entries poe.ninja sends without `divineValue` (under ~0.5c) get one from the divine rate; the live run hit this.
- **Samples:** real item texts come from Sidekick's tests (MIT, credited in `tests/data/items/SOURCE.md`), including Korean ones for the English-only message.
- **Found on the way:** the launch tests started real overlays with the player's environment. These rewrote `~/.config/niri/poe-overlay.kdl` with the test binary's path, and could touch saved progress. They now run with temporary config and state folders and no niri session.
- **Checked live** with a stand-in game: a terminal titled "Path of Exile" that puts an item text on the clipboard (`wl-copy`) when it receives Ctrl+Alt+C. The virtual keyboard's keys reached it. The Panel showed Carcass Jack 6L (394c, 5 listed) on the right half. A second press on it closed the Panel. A press on Kinetic Blast of Clustering level 4 showed the 3 closest gem entries. Currency Rates then replaced the Panel.
- **After review**, a second live run checked the new close behaviour. Carcass Jack showed. A second press closed it. The Saint's Treasure showed. A press with nothing copied left it closed. That session also found a bug, now fixed: a press pressed Ctrl+Alt+C even when the game wasn't focused, so the keys went to whatever window had focus. Price Check now does nothing unless the game is focused.

**Needs an in-game check (human):**

| Check | Pass when | Result |
|---|---|---|
| Copy reaches the overlay | Alt+D over a unique, a currency stack, a gem and a card each shows its price (Proton Experimental) | |
| Ctrl-based binding | With `price-check = "Ctrl+D"`, holding Ctrl and pressing D twice over two items prices both, and Ctrl still works in game afterwards | |
| Placement | Pointer on the left half: Panel on the right; pointer on the right half: Panel next to the Route Panel | |
| Closing | ✕, Alt+Tab, Alt+D again on the same item, Alt+D with nothing hovered each close it | |
| Clipboard | Afterwards your clipboard holds the item text, as when you copy it yourself | |
