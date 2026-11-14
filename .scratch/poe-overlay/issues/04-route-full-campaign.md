# 04: Route covers the whole campaign

**What to build:** Extend the Route from Act 1 to every act of the campaign, including exile-leveling's league-start variant, with the exile-leveling MIT copyright notice bundled and shown in the about/credits section. Steps for every act display and advance the same way as in Act 1.

**Blocked by:** 03 (Route follows Area changes)

**Status:** ready-for-agent

- [ ] All acts' Route data is bundled and parses without errors (verified by a test that loads every act)
- [ ] League-start variant is selectable and changes the Steps shown where the data differs
- [ ] exile-leveling copyright notice ships with the program
- [ ] Entering an Area in a later act (e.g. Act 6 coast) shows that act's Steps, not Act 1's

## Comments

### Note from ticket 03 (agent, 2026-11-14)

The Route parser understands only the fragments Act 1 uses. Acts 2–10 (exile-leveling `d6951db`) also use `{portal|set}` / `{portal|use}` (9×), `{crafting}` (47×) and `{ascend}` (3×). `{portal|use}` must move the simulated Area the way exile-leveling does, or Steps will be grouped under the wrong Area.
