# 21: Sale type filter and listing kinds

**What to build:** The wide Price Check Panel gets a sale type drop-down: *Instant buyout* (Merchant's Tab listings, bought by travelling to the seller's hideout, seller online or not), *In person* (sellers online now, reached by whisper), or *Both*, the default. The choice feeds the next Search. Each Listing row shows its kind; AFK shows only on in-person rows. GLOSSARY gets "Instant buyout" and "In person" as Listing kinds.

**Blocked by:** 19 (Wide Price Check Panel with switchable filters)

**Status:** ready-for-human

- [x] The first search of a check covers both kinds
- [x] Choosing Instant buyout or In person and pressing Search shows only that kind
- [x] Every Listing row shows whether it is instant buyout or in person
- [x] AFK never shows on an instant-buyout row

## Comments

### Implemented (agent, 2026-11-16)

- **Control:** a row of linked toggle buttons, *Both | Instant buyout | In person*, instead of a drop-down (user's choice: a drop-down's popup would grab the keyboard on Wayland while open). The chosen one shows in the Panel's gold. Picking alone sends nothing; Search uses it.
- **Search:** the trade site's status option: `available` (both, the default for the first search), `securable` (instant buyout), `online` (in person, sellers online now). The first search used to ask for online sellers only.
- **Listings:** an instant-buyout listing is the one with a gold `fee` (recorded 2026-11-16; Awakened PoE Trade tells them apart the same way). Each row's last column says `instant`, `in person` or `AFK`; AFK only for in-person sellers. The count reads "N listed" (instant-buyout sellers may be offline); the trade site caps it at 10000.
- **Tests:** `first_search_asks_for_cheapest_non_unique_listings_of_both_kinds` (the old "online" contract test, updated), `sale_type_picks_instant_buyout_or_in_person_listings` (`tests/trade_query.rs`), `instant_buyout_listings_are_told_from_in_person_ones_and_are_never_afk` (`tests/trade_search.rs`, on a fetch recorded with both kinds, `tests/data/trade-fetch-available.json`); each fails when the code it covers is broken.
- **Checked live:** Sorcerer Boots opened with *Both* on: "10000 listed", ten `instant` rows at 1 alch. Clicking *In person* then Search: "957 listed", ten rows at 1 chaos, nine `in person` and one `AFK`. The game kept keyboard focus.
- **README/GLOSSARY:** Listing kinds and the Sale choice.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Instant buyout | Pick *Instant buyout*, Search: only `instant` rows, and the cheapest is one you can travel to on the trade site | |
