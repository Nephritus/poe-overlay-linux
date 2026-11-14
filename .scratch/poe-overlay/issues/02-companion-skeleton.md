# 02: Companion skeleton: launch with PoE, show only while the game is focused

**What to build:** Running `poe-overlay launch -- %command%` as the Steam launch option starts the game and, if one isn't already running, the overlay. The overlay is single-instance and exits when the game exits, so the user never starts or stops it manually. A placeholder panel is visible only while the PoE window is focused, on the monitor the game is on; switching away hides it, returning shows it again. The overlay never holds keyboard focus; clicks outside panels pass through. Focus tracking targets niri first (foreign-toplevel activated state); other compositors come in ticket 15.

**Blocked by:** 01 (Toolkit prototype)

**Status:** ready-for-agent

- [ ] Launch option starts the game normally (exit code and arguments passed through) and starts exactly one overlay even if launched twice
- [ ] Overlay exits when the game process exits
- [ ] Placeholder panel appears only while the PoE window is focused and hides on Alt+Tab
- [ ] Panel appears on the output the game window is on
- [ ] Clicking the panel leaves keyboard focus on the game; clicks outside it reach the game
- [ ] Built with the toolkit chosen in 01
