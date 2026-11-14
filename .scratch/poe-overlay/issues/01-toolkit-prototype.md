# 01: Toolkit prototype: GTK4 vs iced click-through panel over PoE

**What to build:** Build two throwaway prototypes, one with gtk4-rs + gtk4-layer-shell and one with iced + iced_exwlshell, each showing a transparent layer-shell panel with one button over a running Path of Exile on niri. The point is to prove the core guarantee this project exists for (the overlay never steals focus from the game and never blocks clicks outside its panels) and to let the user pick the toolkit they prefer working in. If both pass and preference is tied, choose GTK4.

Awakened PoE Trade failed on this exact setup: overlay not on top, focus stealing/flicker, dead hotkeys, broken copy, and a black screen after Alt+Tab (absent when it isn't running). Sourced facts behind these decisions are in this feature's `research.md`. Vocabulary per `GLOSSARY.md`.

**Blocked by:** None (can start immediately)

**Status:** ready-for-human

- [ ] Each prototype shows a semi-transparent panel with a button on the overlay layer above PoE (borderless windowed, Proton/Xwayland) on niri
- [ ] Clicking the button registers in the prototype while the PoE window keeps keyboard focus (confirmed via the compositor's focused-window query and by game keys still working)
- [ ] Clicks outside the panel reach the game
- [ ] Alt+Tab away and back shows the game normally: no black screen, no flicker
- [ ] A short comparison note (code feel, look, dependencies, AppImage implications) is appended to this ticket
- [ ] The user's toolkit choice is recorded in this ticket; prototype code is deleted or kept out of the main source tree
