# 03: Route follows Area changes (Act 1)

**What to build:** The overlay locates Client.txt via Steam's library list (app 238960) and follows it live. A Route panel shows the current Steps for the Area the Character is in, using exile-leveling's Act 1 Route data, and advances automatically when Client.txt reports entering a new Area. Areas are identified by the game's area ID (from the `Generating level N area "ID"` line), not display name, so same-named Areas in different acts resolve correctly. Only lines written after the overlay starts drive progress; old log history must not replay through the Route.

**Blocked by:** 02 (Companion skeleton)

**Status:** ready-for-human

- [x] Client.txt is found automatically for a Steam install in any Steam library folder
- [ ] Entering an Act 1 Area moves the Route panel to that Area's Steps within a second
- [x] Same-named Areas are distinguished by area ID
- [x] Existing log content at startup does not advance the Route
- [x] Log parsing is covered by tests using recorded Client.txt lines

## Comments

### Implemented (agent, 2026-11-14)

The code is on `main`. What's covered:
- **Client.txt lines:** `Generating level N area "ID"` counts as entering an Area; it's logged about a second before `You have entered`. Tested with lines recorded from a real session.
- **Route:** exile-leveling's Act 1 Route (league-start variant, its site's default) parsed into Legs: the Steps for one visit to an Area. On entering an Area, the panel moves to that Area's Steps. Returning from a detour (a town trip, stepping back) goes back to the Leg it left; otherwise the nearest Leg wins, ahead on a tie. Areas the Route doesn't visit keep the last Steps and say so.
- **Log following:** starts at the end of the file, so history isn't replayed. Handles half-written lines, CR and CRLF endings, and truncation or replacement. Polls every 250 ms.
- **Finding Client.txt:** through `libraryfolders.vdf` and `appmanifest_238960.acf`. Against the real install, it found `~/.local/share/Steam/steamapps/common/Path of Exile/logs/Client.txt`.
- **Replay:** the real session's Area lines, appended to a followed file, each reached the Route within 250 ms.

**Needs an in-game check (human):**

| Check | Pass when | Result |
|---|---|---|
| Panel shows Steps | The Route panel shows the Area name and its Steps while PoE is focused | |
| Follows Areas | Entering a new Act 1 Area updates the panel within about a second | |
| Readable | Text wraps inside the panel; arrows (←, →) render | |
| Click area | Clicks next to the panel still reach the game after the panel changes size | |

**Known limits:**
- **Progress isn't saved between sessions (ticket 05):** a Character partway through Act 1 who logs in to town sees the first town visit's Steps until entering the next Area.
- **Boss arenas:** `{arena}` Steps (The Warden's Quarters, Merveil's Lair) have no area ID in the Route data. If the game logs one, the panel shows "off the Route" there with the last Steps kept.
