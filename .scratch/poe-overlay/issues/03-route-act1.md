# 03: Route follows Area changes (Act 1)

**What to build:** The overlay locates Client.txt via Steam's library list (app 238960) and follows it live. A Route panel shows the current Steps for the Area the Character is in, using exile-leveling's Act 1 Route data, and advances automatically when Client.txt reports entering a new Area. Areas are identified by the game's area ID (from the `Generating level N area "ID"` line), not display name, so same-named Areas in different acts resolve correctly. Only lines written after the overlay starts drive progress; old log history must not replay through the Route.

**Blocked by:** 02 (Companion skeleton)

**Status:** ready-for-agent

- [ ] Client.txt is found automatically for a Steam install in any Steam library folder
- [ ] Entering an Act 1 Area moves the Route panel to that Area's Steps within a second
- [ ] Same-named Areas are distinguished by area ID
- [ ] Existing log content at startup does not advance the Route
- [ ] Log parsing is covered by tests using recorded Client.txt lines
