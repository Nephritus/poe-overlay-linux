# 02: Companion skeleton: launch with PoE, show only while the game is focused

**What to build:** Running `poe-overlay launch -- %command%` as the Steam launch option starts the game and, if one isn't already running, the overlay. The overlay is single-instance and exits when the game exits, so the user never starts or stops it manually. A placeholder panel is visible only while the PoE window is focused, on the monitor the game is on; switching away hides it, returning shows it again. The overlay never holds keyboard focus; clicks outside panels pass through. Focus tracking targets niri first (foreign-toplevel activated state); other compositors come in ticket 15.

**Blocked by:** 01 (Toolkit prototype)

**Status:** ready-for-human

- [x] Launch option starts the game normally (exit code and arguments passed through) and starts exactly one overlay even if launched twice
- [x] Overlay exits when the game process exits
- [ ] Placeholder panel appears only while the PoE window is focused and hides on Alt+Tab
- [ ] Panel appears on the output the game window is on
- [ ] Clicking the panel leaves keyboard focus on the game; clicks outside it reach the game
- [x] Built with the toolkit chosen in 01

## Comments

### Implemented (agent, 2026-11-14)

The code is on `main`. The automated tests cover the three ticked items:
- **Launcher contract:** exit code, arguments, Steam's environment and `LD_PRELOAD` reach the game unchanged. The overlay runs without `LD_PRELOAD`. Launching twice runs one overlay, and the overlay exits when the game exits.
- **Game-focus logic:** the panel shows on activate, hides on Alt+Tab, hides on close, follows the game to another output, and isn't fooled by a browser tab about PoE.

**Live smoke on niri 26.04,** using a stand-in window titled "Path of Exile" instead of the game:
- The `poe-overlay` layer surface was mapped, with keyboard interactivity `none`, only while the stand-in was focused.
- The overlay process exited when the stand-in did.

**Needs an in-game check (human).** Set the Steam launch option to `<path>/poe-overlay launch -- %command%`, start PoE (borderless windowed), then:

| Check | Pass when | Result |
|---|---|---|
| Panel appears | The "PoE Overlay" panel shows at the left while PoE is focused | Pass (user, 2026-11-14) |
| Window identity | `niri msg focused-window` on PoE shows app_id `steam_app_238960` (title "Path of Exile" also matches) | |
| Alt+Tab | Switching away hides the panel; switching back shows it, with no black screen or flicker | |
| Focus kept | Clicking the panel leaves PoE focused, and game keys still work | |
| Click-through | Clicking next to the panel reaches the game | |
| Exit with game | After quitting PoE, `pgrep -af "poe-overlay overlay"` prints nothing and Steam shows the game as stopped | |
| Game's monitor | Multi-monitor only: the panel shows on PoE's monitor | n/a (single screen) |

### In-game notes (2026-11-14)

- **Starting from Steam:** works when the launch option resolves to the binary. A bare `poe-overlay` needs the program on Steam's PATH, which includes `~/.cargo/bin` (e.g. after `cargo install --path .`). Otherwise Steam logs `/bin/sh: poe-overlay: command not found` and the game doesn't start.
- **Cursor blocked during actions:** while a mouse button is held for an in-game action, the cursor sometimes stops at the panel's edge. The PoE config has `confine_mouse_to_window=false`, and the cursor enters the panel normally once no button is held.
  - Likely cause (not confirmed): while a button is held, the game locks the pointer to its window, and niri won't let a locked pointer move onto another surface.
  - Harmless, since panels can't be clicked mid-action anyway.
  - If the cursor is blocked with no buttons held, it's a different problem. The candidate fix is Wine's X11 `GrabPointer=N` in the game's Proton prefix.
