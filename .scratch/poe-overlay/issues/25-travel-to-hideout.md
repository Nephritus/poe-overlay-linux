# 25: Travel to Hideout from an instant-buyout listing

**What to build:** Each instant-buyout Listing row in the Price Check Panel gets **Travel to Hideout**. One click asks the trade site, with the player's login, to send the player to that seller's hideout, the same request the trade site's own button sends; the game then teleports the player there. Each click sends exactly one request and nothing is sent without a click. Without a login the button says to log in and opens Settings. A refused or failed request shows on the row, with the trade site's reason when it gives one.

**Blocked by:** 21 (Sale type filter and listing kinds), 23 (Log in with a Firefox-family browser)

**Status:** ready-for-human

- [ ] Clicking Travel to Hideout on an instant-buyout row teleports the player in game (human check: not sent from here, it would act on the user's account)
- [x] One click sends one request; in-person rows have no such button
- [x] Without a login the button leads to Settings and sends nothing
- [x] A failed request shows its reason on the row

## Comments

### Implemented (agent, 2026-11-17)

- **The request (from the trade site's own page script, 2026-11-17):** `POST /api/trade/whisper` with `{"token": hideout_token}`, sent with `X-Requested-With: XMLHttpRequest` and `Origin` (now on every POST) and the login. `{success:true}` → teleporting; `{success:false}` → "Item is in demand", after which the site's next click adds `"continue": true` (done the same, user's choice); errors as `{error:{code,message}}`, also with HTTP 200. Neither Awakened PoE Trade nor Sidekick implements it.
- **Tokens:** `hideout_token` is only in logged-in fetches (checked: absent logged out) and is a JWT that expires about 300 s after the fetch. Its `exp` is read (small base64url decoder, no new crate); an expired one isn't sent: "This Listing is too old to travel to: press Search for a fresh one." (user's choice), and the same search then fetches its Listings again. `HideoutToken`'s `Debug` leaves the token out.
- **`TradeSource::travel`:** one request per call within the rate limits the site states for that endpoint (its own `RateLimits`); nothing sent logged out (`TradeError::LoggedOut`) or while the site refuses searches. A 403 on travel is a failure with the reason, not a 5-minute search block.
- **Listings and the login:** the same search within 5 minutes reuses its Listings only if the login is as it was at the fetch: after logging in (setting or browser), Search fetches again, now with ways to travel.
- **Panel:** each instant-buyout row gets **Travel to Hideout**; fetched logged out it says **Log in** and opens the Settings window (`settings_window::open`, moved from `overlay.rs`). Outcomes show on a line under the row: "Teleporting…", in demand, too old, the site's reason. Buttons are pointer-only.
- **Tests:** `tests/trade_travel.rs` (9): contact fields per Listing kind from a recorded fetch (tokens replaced by same-shape fakes); body, login and one request per call; in demand then `continue`; nothing sent logged out or expired; the site's reason for 400, error-in-200, 500 and 403, searches not held back; travel rate limits kept apart; the same search after logging in or out, after a login turns up in the browser, and after an expired token fetches again. Mutation-checked (one equivalent survivor: base64url `_` → 62 still decodes `exp`).
- **Checked live** (stand-in game, user's real Firefox login): a Price Check of Invasion Track showed Travel on instant rows and Whisper on in-person rows; ~5 min after the search, Travel showed "This Listing is too old…" (no request); logged out, the rows said Log in and a click opened the Settings window. No travel request was sent.
- **After review:** fixed the token leaking through `Debug`, the stale "Log in" rows after logging in in the browser, the travel 403 saying "search" and blocking searches, a clock ahead of the site's leaving an expired Listing reused; the button reads "Travel to Hideout" as the ticket says. Not changed: "whisper" stays the field name (it's the chat message itself); `base64url` stays beside its only user.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Travel | Logged in and in game, Travel to Hideout on an instant buyout teleports you to the seller's hideout | |
| In demand | When the row says the item is in demand, the next click travels | |
