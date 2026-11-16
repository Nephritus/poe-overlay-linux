# 20: Typed minimum and maximum

**What to build:** Each stat row in the wide Price Check Panel gets a minimum and a maximum box the player types into, like the trade site. Clicking a box lets the Panel take the keyboard; **Enter** runs the search and gives the keyboard back to the game, **Esc** gives it back without searching, and closing the Panel (✕, the Price Check hotkey, or another Panel taking its place) always gives it back. An empty box means no bound. Until the keyboard is given back, keys go to the Panel, not the game, and the Panel makes that visible. The Overlay rule "never takes keyboard focus" becomes "takes it only while the player is typing in a box"; README and GLOSSARY say so.

**Blocked by:** 19 (Wide Price Check Panel with switchable filters)

**Status:** ready-for-human

- [x] Typing a minimum and pressing Enter searches with it and returns keys to the game
- [x] A maximum limits the search the same way; an empty box adds no bound
- [x] Esc returns keys to the game without sending a request
- [x] Closing the Panel while typing returns keys to the game
- [x] Non-numeric input can't be searched
- [x] Without clicking a box, the game never loses a key press

## Comments

### Implemented (agent, 2026-11-16)

- **Spike first (niri 26.04):** the Overlay's layer surface switches to *exclusive* keyboard mode when a box is clicked and back to *none* on Enter, Esc, Search, closing the Panel or another Panel taking its place. Switching to *on-demand* after taking the keys dropped them at once, so clicking back into the game doesn't give keys back; Enter, Esc or closing does. While the Overlay holds the keys niri reports no window focused, so focus tracking now tells "no window focused" (`Visibility::NoWindowFocused`) from "another window focused": the first keeps the Overlay shown while typing, otherwise it hides as before. Another window taking focus mid-typing hides the Overlay and gives the keys back. A Price Check trigger while typing does nothing (the copy keys would go to the Overlay).
- **Panel:** each stat row has a min box (filled with the 90% value) and a max box (empty). Clicking a box selects its value, so typing replaces it. "Typing: Enter searches, Esc gives the keys back to the game." shows under Search while the keys are held. A bound that isn't a number turns red, the right column says so, nothing is sent and the keys stay.
- **Search:** each filter carries an optional max; an empty side is left out of the request.
- **Tests:** `no_window_focused_while_the_game_is_open_is_told_apart_from_another_window_focused` (`tests/game_focus.rs`), `typed_bounds_are_numbers_or_nothing`, `typed_minimum_and_maximum_are_searched_and_an_empty_side_left_out` (`tests/trade_query.rs`); each fails when the code it covers is broken.
- **Checked live** (stand-in game logging every key it gets, clicks and keys through virtual devices): typing `80w` went into the box with the Overlay still shown and nothing reaching the game; Enter searched (Search greyed, then "1 listed online · 10 chaos" for an edited search) and the next key reached the game; junk was marked red with keys kept; Esc and ✕ each gave the keys back; clicking a box holding 65 and typing 50 left 50.
- **README/GLOSSARY:** the Overlay takes the keyboard only while the player types in a bound box.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Typing in game | Click a min box over the real game, type a number, Enter: the listings change and your next key moves your character | |
| Esc | Click a box, press Esc: the game takes keys again at once | |
| Alt-tab while typing | Click a box, switch to another window: the Overlay hides; back in the game, keys reach the game | |
