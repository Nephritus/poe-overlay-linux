# 06: Currency Rates panel toggled by hotkey

**What to build:** Introduce the hotkey path: `poe-overlay trigger <action>` sends an action to the running overlay over a local socket, and the user binds it in their compositor (docs include the niri bind snippet). The first action toggles a Currency Rates panel listing common currencies priced in chaos and divine, fetched from poe.ninja's documented economy endpoints with an identifying User-Agent and a 15-minute cache. The league defaults to the current challenge league.

**Blocked by:** 02 (Companion skeleton)

**Status:** ready-for-agent

- [ ] `poe-overlay trigger currency-rates` toggles the panel while the game is focused
- [ ] Trigger with no running overlay exits non-zero with a clear message
- [ ] poe.ninja is called at most once per 15 minutes per league; requests carry a contact User-Agent
- [ ] Panel shows prices in chaos and divine for the default league
- [ ] Network failure shows a readable error in the panel, not a crash
- [ ] README documents the niri bind snippet
