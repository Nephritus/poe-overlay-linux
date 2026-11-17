# 27: Move Panels by dragging their border

**What to build:** Some Panels cover game buttons. The player drags a Panel by its border to move it out of the way. Positions aren't remembered: every start puts Panels in their usual places (user's choice).

**Blocked by:** None

**Status:** ready-for-human

- [x] Pressing a Panel's border and dragging moves it; elsewhere on the Panel, presses act as before
- [x] Clicks where a moved Panel used to be reach the game
- [x] A dragged Panel stays where it was put until the Overlay restarts

## Comments

### Implemented (agent, 2026-11-17)

- **Grip:** the outer 6 px of a shown Panel (inside its 12 px padding, clear of its controls; the Trade Requests stack's outer card edges), the topmost Panel first. One `GestureDrag` and one motion controller on the Overlay's `Fixed` (capture phase), in `OverlaySurface`, so every Panel gets it. Over a border the cursor is `move`. The keyboard stays with the game.
- **Clamped wholly on the output** (its monitor's size): a Panel sticking out made the Overlay window grow past the output (2253×1128 seen), so "keep 48 px on screen" was dropped.
- **Dragged Panels stay put:** `move_panel` leaves them alone, so Price Check stops jumping to the half away from the pointer and Trade Requests stops re-centring, until restart.
- **Bug found while building:** `Fixed::child_position` reports the content box (13 px more than where the Panel was put) and `widget.width()` excludes padding and border; the drag uses `compute_bounds` instead.
- **Tests:** none (UI, checked live as agreed for Panels).
- **Checked live** (stand-in game with terminal mouse reporting, uinput pointer): Route Panel dragged by its left border from (40, 200) to (1100, 700); a press-and-drag inside it didn't move it; a click on its old spot reached the game; ▶ on the moved Panel advanced the Step; a drag far past the bottom right stopped with the Panel wholly on screen. Price Check dragged to (100, 500); the next Price Check with the pointer on the other half stayed there. A Trade Request card dragged to the right; the next whisper's card stacked there. Move cursor over a border, arrow inside. The game kept the keyboard (the second Price Check's key presses reached it).

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| In game | The Route Panel dragged off the stash lets you click the stash behind its old spot | |
| Mouse | Dragging works with your mouse and cursor theme (move cursor shows on a border) | |
