# 05: Record Hotkeys, applied live

**What to build:** A Hotkeys section in the Menu and the Settings window lists each Action with its key. Clicking a key box and pressing a combination saves it. A combination another Action uses is refused, with a message naming that Action. Esc cancels; Backspace clears (no Hotkey). The running Overlay lets go of the old keys and grabs the new ones at once. While recording in the Menu, the Overlay holds the keyboard, so the combination doesn't reach the game.

**Blocked by:** 04 (The Menu)

**Status:** ready-for-agent

- [ ] Recording Ctrl+F7 for Currency Rates writes it to `[hotkeys]` and Ctrl+F7 toggles the Panel without a restart; the old key reaches the game again
- [ ] A duplicate is refused and names the Action holding it; nothing is written
- [ ] Esc leaves the key unchanged; Backspace clears it and the Action has no Hotkey
- [ ] Keys a layout can't produce, or that another X11 program has grabbed, are reported as today
