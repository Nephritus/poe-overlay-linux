# 05: Route progress per Character with Previous/Next

**What to build:** Route progress belongs to a Character. The Character's name is learned from Client.txt level-up lines; progress is saved per Character and restored after the overlay restarts. Clickable Previous and Next buttons on the Route panel correct the position when auto-advance gets it wrong (e.g. a backtrack). Buttons work without the overlay taking keyboard focus.

**Blocked by:** 03 (Route follows Area changes)

**Status:** ready-for-human

- [ ] Previous/Next move the current Step and the game keeps keyboard focus
- [x] Progress for two different Characters is tracked independently
- [x] Restarting the overlay restores the last Step for the active Character
- [x] Before any Character is identified, the Route still works and attaches to the Character once identified

## Comments

### Implemented (agent, 2026-11-14)

The code is on `main`. Decisions made with the user:
- **Identifying the Character:** Client.txt names it only in level-up lines, and logins don't name it. So at startup the Character from the latest level-up already in Client.txt is assumed and its saved progress restored. The history is read backwards in chunks, so a huge log doesn't slow startup.
- **Switching Characters:** a level-up of another name switches to that Character. The previous one goes back to its Position at its last confirmation (startup or a level-up), since Areas entered in between may belong to either.
- **Unknown Character:** progress made before any Character is known attaches to the Character once it levels up. A returning Character resumes its saved visit of the Area it's in.
- **Previous/Next** (◀ ▶ in the panel header) move one Leg. They're pointer-only buttons (not focusable), and a correction clears any pending detour.
- **Saving:** progress goes to `~/.local/state/poe-overlay/progress.toml` as a Leg plus its Area, so it survives Route variant changes. It's written atomically, only on change. An invalid file is moved aside with a timestamp. A file that can't be read or moved aside is never overwritten.
- **Party play:** another player's level-up may switch Characters. A name pin can come with the settings window (ticket 09).

**Needs an in-game check (human):**

| Check | Pass when | Result |
|---|---|---|
| Buttons keep focus | Clicking ◀ ▶ changes the Steps, and game keys still work right after | |
| Name shown | The panel shows your Character's name under the header | |
| Restored after restart | Quit, start the game again: the panel shows where you left off | |
