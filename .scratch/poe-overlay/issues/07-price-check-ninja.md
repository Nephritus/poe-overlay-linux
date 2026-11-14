# 07: Price Check for poe.ninja-priced items

**What to build:** The Price Check hotkey makes the overlay release any modifiers the user is still holding, send Ctrl+Alt+C to the game via a uinput virtual keyboard, and read the clipboard once through the data-control protocol. The item text is parsed; uniques, currency, gems and divination cards show their poe.ninja price. The panel opens on the screen half opposite the cursor (cursor position queried from the game's Xwayland server; fall back to the left half if unavailable) and closes on its ✕ button, pressing the hotkey again, a new Price Check, or the game losing focus. Non-English item text shows a clear "English client only" error. If /dev/uinput isn't accessible, the overlay shows the udev rule to install instead of failing silently. One clipboard read per key press (GGG's tool rule).

**Blocked by:** 06 (Currency Rates panel toggled by hotkey)

**Status:** ready-for-agent

- [ ] Pressing the bound hotkey over a unique, currency item, gem or divination card shows its poe.ninja price
- [ ] Holding the hotkey's modifiers does not corrupt the copy (verified with a Ctrl-based binding)
- [ ] Panel appears on the half opposite the cursor and closes by each of the four rules
- [ ] Exactly one clipboard read per press
- [ ] Non-English item text and missing uinput access each produce their specific message
- [ ] Item text parsing is covered by tests using real copied item samples
