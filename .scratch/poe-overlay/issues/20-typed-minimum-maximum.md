# 20: Typed minimum and maximum

**What to build:** Each stat row in the wide Price Check Panel gets a minimum and a maximum box the player types into, like the trade site. Clicking a box lets the Panel take the keyboard; **Enter** runs the search and gives the keyboard back to the game, **Esc** gives it back without searching, and closing the Panel (✕, the Price Check hotkey, or another Panel taking its place) always gives it back. An empty box means no bound. Until the keyboard is given back, keys go to the Panel, not the game, and the Panel makes that visible. The Overlay rule "never takes keyboard focus" becomes "takes it only while the player is typing in a box"; README and GLOSSARY say so.

**Blocked by:** 19 (Wide Price Check Panel with switchable filters)

**Status:** ready-for-agent

- [ ] Typing a minimum and pressing Enter searches with it and returns keys to the game
- [ ] A maximum limits the search the same way; an empty box adds no bound
- [ ] Esc returns keys to the game without sending a request
- [ ] Closing the Panel while typing returns keys to the game
- [ ] Non-numeric input can't be searched
- [ ] Without clicking a box, the game never loses a key press
