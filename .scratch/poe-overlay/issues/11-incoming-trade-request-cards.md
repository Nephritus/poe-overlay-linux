# 11: Incoming Trade Request cards

**What to build:** Incoming Trade Requests (trade-site whispers from buyers in Client.txt) appear as cards showing buyer, item, price, and stash tab and position. Cards stack newest on top, at most about five; older ones are pushed out. Each stays until closed with ✕. Plain whispers that aren't Trade Requests do not create cards. Nothing is removed automatically on log events.

**Blocked by:** 03 (Route follows Area changes)

**Status:** ready-for-human

- [x] A trade-site whisper line produces a card with all four fields
- [x] A non-trade whisper produces no card
- [x] A sixth request pushes out the oldest card
- [x] ✕ closes a card and the game keeps focus
- [x] Parsing is covered by tests with real whisper lines, including guild tags and bulk currency requests

## Comments

### Implemented (agent, 2026-11-16)

- **Parsing (`poe1::trade_request`):** `@From [<TAG>] Name:` (incoming) and `@To Name:` (outgoing, shown already; actions come with ticket 13) with the trade site's templates: `Hi, I would like to buy your ITEM [listed for PRICE] in LEAGUE (stash tab "TAB"; position: left L, top T)[NOTE]` and bulk `Hi, I'd like to buy your ITEM for my PRICE in LEAGUE[. NOTE]`. A Trade Request is its own `LogEvent`; the Route ignores it.
- **Real lines:** the user's Client.txt held no whispers, so the tests use the templates the trade site returned on 2026-11-16 (a listing fetch and a bulk currency exchange), with made-up names. The bulk template no longer ends in a full stop (Awakened PoE Trade still expects one); both are read. Whispers in another language (the buyer's client language; seen: Russian) make no card.
- **Panel:** top centre (left/right are the stash and inventory), 320 px, above nothing else but under the Price Check and Currency Panels. Buyers' cards gold, outgoing blue. A card shows who, item, price, stash tab and position, and the note; bulk ones have no stash line. Five at most, the oldest pushed out; ✕ closes one; the Panel hides with no cards. Cards stay while the game loses and regains focus.
- **Tests:** `tests/trade_request.rs` (8): incoming with every field, guild tag and a tab name with a semicolon, outgoing, bulk, bulk with full stop and note, note after the stash spot, unpriced listing, plain whisper / other channel / Russian whisper → none. Each fails when the code it covers is broken (one mutation, `rsplit` vs `split` on the stash marker, was equivalent; the plainer `split` stays).
- **Checked live:** six whispers appended to a temporary Client.txt (plus a plain one): five cards, newest first, the plain whisper ignored; a sixth pushed out the oldest; ✕ on the bulk card closed it with the game still focused and taking keys.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Real buyer | A buyer's whisper in game shows a card with the right item, price and stash spot | |
