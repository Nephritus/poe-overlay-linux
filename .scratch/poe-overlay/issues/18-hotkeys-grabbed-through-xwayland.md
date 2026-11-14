# 18: Hotkeys grabbed through Xwayland while the game is focused

**What to build:** The overlay catches its hotkeys itself, on every compositor, and only while the game is focused. Under Proton the game is an X11 window in Xwayland, so Xwayland receives its keys. While the game has focus, the overlay holds an X11 passive grab (`GrabKey` on Xwayland's root window) for each key in `[hotkeys]`; it releases them when the game loses focus or closes. A grabbed key runs its Action directly and doesn't reach the game. Other apps keep the key: Xwayland gets no keys while a Wayland window is focused, and the grab is gone while another X11 window is.

This replaces the compositor-bind mechanism (clean cutover):
- Remove the niri writer (`src/hotkeys/niri.rs`), `poe-overlay setup hotkeys [--remove]`, the first-launch "Set up hotkeys?" row, and the `hotkeys.toml` answers.
- Keep `[hotkeys]` as the key list.
- Keep `poe-overlay trigger <action>` as the manual fallback, for compositor binds and for a game running without Xwayland.
- Ticket 15's per-compositor hotkey items are dropped.
- Delete the user's generated `~/.config/niri/poe-overlay.kdl` and its `include` line by hand once. The project is unreleased, so no migration code is needed.

**Blocked by:** 17 (Hotkeys set up by the overlay on first launch)

**Status:** ready-for-agent

- [ ] With the game focused, each `[hotkeys]` key runs its Action on niri (xwayland-satellite), and the key doesn't reach the game
- [ ] With a Wayland window or another X11 window focused, the same keys reach that window and run nothing
- [ ] Grabs follow focus: taken when the game gains focus, released when it loses focus or exits
- [ ] Keys work with NumLock and CapsLock on (grab each key with every lock-modifier combination)
- [ ] A key Xwayland refuses (another X11 client already grabbed it) is named on the Route Panel with a pointer to `[hotkeys]`
- [ ] The niri writer, `setup hotkeys`, the first-launch row and `hotkeys.toml` are gone, and README and GLOSSARY ("Hotkey") describe the new mechanism; `trigger` stays documented as the fallback
- [ ] Recorded on at least one compositor with a standard Xwayland (KDE, Hyprland or sway) besides niri

## Comments

### Prototype (agent, 2026-11-15)

Throwaway `x11rb` program on niri 26.04 with xwayland-satellite. It grabbed Alt+K (Mod1, and Mod1+Mod2 for NumLock) on the root of `:1`, opened its own X11 window, and pressed Alt+K through uinput only while niri reported the intended window focused:

| Focused | Result |
|---|---|
| The X11 test window | `KeyPress code=45 state=MOD1\|MOD2 event=root`: the grab fired; the window got only the Alt press, not K |
| A Wayland window (alacritty) | No event |
| The X11 window again | The grab fired again |

`GrabKey` returned no error for either modifier set. NumLock was on, so the event state carried Mod2: the lock variants are required.

Precedent: Awakened PoE Trade's Linux hotkeys go through Electron's `globalShortcut` (`main/src/shortcuts/Shortcuts.ts:132`), which uses X11 key grabs, against the same Wine game window.

**Risks to check in game:**
- Around each grabbed press, the focused X11 window gets `FocusOut`/`FocusIn` with mode `NotifyGrab`/`NotifyUngrab` (seen in the prototype). Wine is expected to treat these as a keyboard grab, not a focus loss; PoE must not pause or drop held keys.
- If Wine itself holds an active keyboard grab (fullscreen grab settings), passive grabs never fire.
- A key bound in the compositor never reaches Xwayland; the overlay can't see that conflict. Document it.
- Proton's native Wayland mode (`PROTON_ENABLE_WAYLAND=1`) bypasses Xwayland. Price Check's pointer lookup has the same limit; `trigger` remains the fallback.

**Needs an in-game check (human):**

| Check | Pass when | Result |
|---|---|---|
| Hotkeys in game | Alt+R and Alt+D work in PoE with no compositor binds installed | |
| No focus blip | Holding a movement key and pressing Alt+D doesn't stop the character or open/close game menus | |
| Other apps | Alt+D in Firefox (Wayland) still focuses the address bar while the game runs | |
