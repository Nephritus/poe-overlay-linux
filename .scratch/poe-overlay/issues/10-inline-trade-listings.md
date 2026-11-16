# 10: Inline trade listings in Price Check

**What to build:** Price Checking an item poe.ninja can't price (rares, magic items, bases, etc.) searches the trade site and shows the cheapest listings inside the Price Check panel, as Awakened PoE Trade does by default. The "Open on trade site" button from 08 stays next to them for refining the search. Requests use a browser User-Agent (Cloudflare rejects identifying ones) and follow the trade site's rate-limit headers. On a 403 or an active rate limit, the panel says why and keeps the button.

Decided 2026-11-15: on by default, no opt-in. The trade endpoints aren't in GGG's API reference, and its developer docs call reverse-engineering unlisted endpoints a Terms of Use 7i violation. The comparable tools search by default anyway (APT: `smartInitialSearch: true`, `PriceCheckWindow.vue:107`), and README states the risk.

**Blocked by:** 08 (Price Check trade site search)

**Status:** ready-for-human

- [x] Price Checking a rare shows its cheapest online listings in the panel (price, listed time, seller status)
- [x] Rate-limit headers (`X-Rate-Limit-Ip` and `-State`) are honoured: no request is sent while a limit is active, and the panel says when it can search again
- [x] A 403 or 429 shows the reason next to the "Open on trade site" button
- [x] README states that the trade endpoints are undocumented and that the overlay searches them on each Price Check of an item poe.ninja can't price

## Comments

### Implemented (agent, 2026-11-16)

- **`poe1::trade_search`:**
  - `TradeSource` sends one search (`POST /api/trade/search/{league}` with ticket 08's query) and one fetch of the first 10 results, and reads price, listing time and AFK sellers. Searches ask for online sellers only, so AFK is the only seller status shown.
  - `RateLimits`, one per endpoint (search and fetch have different policies), reads `X-Rate-Limit-Rules`, each rule and its `-State`, and `Retry-After`. Requests are counted against every rule; a restriction the site reports, a `Retry-After`, or a rule the site counts as full from other clients on the same address blocks until it ends. Nothing is sent while blocked; the Panel says "searching again in 42 s".
  - A 403 stops searches for 5 minutes (GGG counts repeated 4xx answers against the address). The same search within 5 minutes reuses the last answer. Other answers show the site's message.
- **`trade_http`:** ureq with a browser User-Agent; 403 and 429 are answers whose headers are read, not errors.
- **Panel:** up to 10 rows (price, age, AFK), "Trade site · N listed online, cheapest first" or "No online listings like it: loosen the search on the trade site."; problems show in red above the button.
- **Tests:** `tests/trade_limits.rs` (6) and `tests/trade_search.rs` (8) with a search and fetch recorded from the trade site on 2026-11-16 (trimmed). Each new test was checked to fail when the code it covers is broken.
- **Checked live** against the trade site from a stand-in game: Sorcerer Boots (normal) showed "788 listed online" with the 10 cheapest (1 alch … 1 chaos, ages, one AFK); Death Nails showed its one listing (2 chaos) with the made-up modifier listed as not searched.
- **After review (both tickets, fixed in this commit):**
  - Stats on two lines (flasks' "Grants Immunity…") are matched whole; stats that differ only by a number in their wording ("Onslaught for 4 / 10 seconds", "Pierce 2 / # additional Targets") are told apart, the most specific first. Local stats are kept in their own map, so no text is built per lookup.
  - Implicit, crafted and enchant modifiers outside the totals are listed as not searched, so nothing is dropped silently.
  - Waits the site states count from its answer, not from the request. A 429 without `Retry-After` holds searches a minute. A search answered whose fetch was held back by a limit isn't searched again. A press the player has moved on from doesn't send its queued search. Garbled limit headers can't overflow the clock.
  - "you can search again in …" (nothing retries by itself).
  - Re-checked live: Honour Leash showed "No online listings like it" with its enchant, implicit and crafted modifiers as not searched; the unidentified Ornate Mace showed 104 listed online with 10 rows.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Rare in game | Alt+D over a rare shows listings within a few seconds | |
| Many checks | Price checking rares quickly (6+ in 10 s) shows the rate-limit message instead of errors, and searches again after it | |
