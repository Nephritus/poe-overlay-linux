# 16: Fix the three known UI bugs

**What to build:** Three problems left over from v0.1.0:

- The Price Check Hotkey doesn't close the Panel while the player types in a minimum or maximum (its key grab is off while the Overlay holds the keyboard).
- An in-person Listing from an AFK seller says only "AFK".
- An item with neither a known base nor a class shows an empty Item row.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Pressing the Price Check Hotkey while typing in a bound closes the Panel and gives the keys back to the game
- [ ] An AFK in-person row reads "in person · AFK"
- [ ] An item with neither base nor class shows no Item row
