# 10: Inline trade listings in Price Check

**What to build:** Price Checking an item poe.ninja can't price (rares, magic items, bases, etc.) searches the trade site and shows the cheapest listings inside the Price Check panel, as Awakened PoE Trade does by default. The "Open on trade site" button from 08 stays next to them for refining the search. Requests use a browser User-Agent (Cloudflare rejects identifying ones) and follow the trade site's rate-limit headers. On a 403 or an active rate limit, the panel says why and keeps the button.

Decided 2026-11-15: on by default, no opt-in. The trade endpoints aren't in GGG's API reference, and its developer docs call reverse-engineering unlisted endpoints a Terms of Use 7i violation. The comparable tools search by default anyway (APT: `smartInitialSearch: true`, `PriceCheckWindow.vue:107`), and README states the risk.

**Blocked by:** 08 (Price Check trade site search)

**Status:** ready-for-agent

- [ ] Price Checking a rare shows its cheapest online listings in the panel (price, listed time, seller status)
- [ ] Rate-limit headers (`X-Rate-Limit-Ip` and `-State`) are honoured: no request is sent while a limit is active, and the panel says when it can search again
- [ ] A 403 or 429 shows the reason next to the "Open on trade site" button
- [ ] README states that the trade endpoints are undocumented and that the overlay searches them on each Price Check of an item poe.ninja can't price
