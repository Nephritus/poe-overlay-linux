# 06: Currency Rates panel toggled by hotkey

**What to build:** Introduce the hotkey path: `poe-overlay trigger <action>` sends an action to the running overlay over a local socket, and the user binds it in their compositor (docs include the niri bind snippet). The first action toggles a Currency Rates panel listing common currencies priced in chaos and divine, fetched from poe.ninja's documented economy endpoints with an identifying User-Agent and a 15-minute cache. The league defaults to the current challenge league.

**Blocked by:** 02 (Companion skeleton)

**Status:** ready-for-human

- [ ] `poe-overlay trigger currency-rates` toggles the panel while the game is focused
- [x] Trigger with no running overlay exits non-zero with a clear message
- [x] poe.ninja is called at most once per 15 minutes per league; requests carry a contact User-Agent
- [x] Panel shows prices in chaos and divine for the default league
- [x] Network failure shows a readable error in the panel, not a crash
- [x] README documents the niri bind snippet

## Comments

### Implemented (agent, 2026-11-14)

The code is on `main`. It starts with a prefactor from an architecture review: an overlay surface module that holds Panels (`3efe733`).
- **Trigger:** `poe-overlay trigger currency-rates` sends the Action over a Unix socket in `$XDG_RUNTIME_DIR`. The same module owns the single-instance lock. With no overlay running, it exits 1 with a message. A socket left behind by a crash is ignored. The trigger waits at most 2 s for an answer.
- **Prices** come from poe.ninja's exchange overview, for the league poe.ninja lists first unless `[prices] league` overrides it. A league is fetched at most once per 15 minutes. **Deviation, chosen by the user:** after a failure it retries after 1 minute, not 15. Requests use the User-Agent `poe-overlay-linux/<version> (contact: https://github.com/Nephritus)`.
- **The Panel** lists the 12 most traded currencies by poe.ninja volume, in chaos and divine. It sits at 420,200 (moved from the agreed 400 so it clears the Route Panel's padding). It closes when the game loses focus, and cached prices show without a "Loading…" flash.
- **Live check:** the HTTP adapter fetched Covens (12 rates in 320 ms). A trigger to a running overlay was answered, and after the game exited the trigger failed with the message.

**Needs an in-game check (human):**

| Check | Pass when | Result |
|---|---|---|
| Alt+R opens | With the niri bind in place, Alt+R in game shows the Currency Rates Panel with prices | |
| Alt+R closes | Pressing it again hides the Panel | |
| Clicks pass | Clicks outside both Panels reach the game, with the Currency Rates Panel open | |
| Alt+Tab | The Panel is closed when you come back | |
| Offline | With the network off, the Panel shows a readable error instead of crashing | |
