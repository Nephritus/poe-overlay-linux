# 04: Route covers the whole campaign

**What to build:** Extend the Route from Act 1 to every act of the campaign, including exile-leveling's league-start variant, with the exile-leveling MIT copyright notice bundled and shown in the about/credits section. Steps for every act display and advance the same way as in Act 1.

**Blocked by:** 03 (Route follows Area changes)

**Status:** ready-for-human

- [x] All acts' Route data is bundled and parses without errors (verified by a test that loads every act)
- [x] League-start variant is selectable and changes the Steps shown where the data differs
- [x] exile-leveling copyright notice ships with the program
- [x] Entering an Area in a later act (e.g. Act 6 coast) shows that act's Steps, not Act 1's

## Comments

### Note from ticket 03 (agent, 2026-11-14)

The Route parser understands only the fragments Act 1 uses. Acts 2–10 (exile-leveling `d6951db`) also use `{portal|set}` / `{portal|use}` (9×), `{crafting}` (47×) and `{ascend}` (3×). `{portal|use}` must move the simulated Area the way exile-leveling does, or Steps will be grouped under the wrong Area.

### Implemented (agent, 2026-11-14)

The code is on `main`. What's covered:
- **All ten acts** are bundled and parsed as one continuous simulation. Portal, crafting, ascend, logout and waypoint follow exile-leveling's rules.
- **Problem reporting:** the parser reports unknown fragments, Areas and quests, unbalanced `#ifdef`, and portal misuse. A test checks that every variant (league start × library × 4 bandits) parses with zero problems.
- **Variants** come from `~/.config/poe-overlay/config.toml` under `[route]`: `league_start`, `bandit` and `library`, with exile-leveling's defaults. A mistake in the file shows as a note on the Route panel, and the defaults are used.
- **Panel header** reads "Act N · Area" with the Route's act; Aspirants' Plaza shows Act 4, 7 or 10, not its data's Act 3.
- **Credits:** `poe-overlay credits` prints the exile-leveling credit and its MIT license.
- **Starting mid-campaign:** the first Area entered places the Character without counting as a detour, so the first town trip returns correctly.

**Needs an in-game check (human)**, when a Character reaches those acts:
- Act 6–10 area IDs in `Client.txt` match exile-leveling's (e.g. Act 6's Coast is `2_6_2`).
- After the Labyrinth, ascending returns you to the town the Route expects (Sarn in Act 4, the Bridge Encampment in Act 7, Oriath Docks in Act 10).
- Sailing at the end of Act 5 lands in `2_6_town`.
- Taking a portal back into an Area you've already visited still logs `Generating level`.
