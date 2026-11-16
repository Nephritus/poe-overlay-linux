# 25: Travel to Hideout from an instant-buyout listing

**What to build:** Each instant-buyout Listing row in the Price Check Panel gets **Travel to Hideout**. One click asks the trade site, with the player's login, to send the player to that seller's hideout, the same request the trade site's own button sends; the game then teleports the player there. Each click sends exactly one request and nothing is sent without a click. Without a login the button says to log in and opens Settings. A refused or failed request shows on the row, with the trade site's reason when it gives one.

**Blocked by:** 21 (Sale type filter and listing kinds), 23 (Log in with a Firefox-family browser)

**Status:** ready-for-agent

- [ ] Clicking Travel to Hideout on an instant-buyout row teleports the player in game
- [ ] One click sends one request; in-person rows have no such button
- [ ] Without a login the button leads to Settings and sends nothing
- [ ] A failed request shows its reason on the row
