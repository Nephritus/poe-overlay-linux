# 19: Wide Price Check Panel with switchable filters

**What to build:** For rare, magic and normal items, Price Check opens a wide Panel (about twice today's width) laid out like the trade site: the search on the left, the Listings on the right. The left column lists each stat the search uses (pseudo totals and single Modifiers), each with a checkbox and its minimum, followed by the Modifiers left out of the search ("no such stat"; "implicit, crafted or enchant") and a **Search** button. The right column shows "N listed online" and the cheapest Listings, as today. The first search still runs on its own when the Panel opens; unticking or ticking a stat changes nothing until **Search** is pressed, which runs the edited search and refills the right column, within the same rate limits as today. The "Open on trade site" button goes away. Items poe.ninja prices keep the narrow Panel. The wide Panel still opens on the half of the screen away from the pointer and fits within half of a 1920 px screen.

**Blocked by:** None (can start immediately)

**Status:** ready-for-human

- [x] A rare shows its searched stats with minimums on the left and its Listings on the right
- [x] Unticking a stat and pressing Search shows Listings found without that stat
- [x] Ticking or unticking alone sends no request
- [x] "Open on trade site" is gone; uniques and currency still open the narrow Panel
- [x] Rate-limit and refusal messages still show, and a rate-limited Search sends nothing
- [x] All controls leave keyboard focus with the game

## Comments

### Implemented (agent, 2026-11-16)

- **Search:** each filter carries the trade site's wording (pseudo totals hard-coded with their ids, explicit stats from the snapshot, two-line stats unescaped) and can be switched off; switched-off filters are left out of the search. An edited search is a new search, so the 5-minute reuse never hides an edit.
- **Panel:** for rare, magic and normal items it opens about 640 px wide. Left: one row per stat (checkbox with the wording, minimum), **Search**, then the not-searched Modifiers. Right: "N listed online" and the cheapest Listings. Search reads the checkboxes, greys out while the search is out, and the old Listings go while it runs, so an error never sits above them. A Search counts like a press: a later press or Search drops its answer. "Open on trade site" and its link builder are gone. Other items keep the narrow Panel.
- **Layout note:** the two columns are a `gtk::Grid` with fixed widths (380 + 220). A horizontal `gtk::Box` reported its minimum height with each column at its narrowest width (GTK warns "natural size must be >= min size"); the Fixed holding the Panels allocates that minimum, so the Panel came out ~170 px too tall.
- **Tests:** `filters_carry_the_trade_site_wording`, `switched_off_filters_are_left_out_of_the_search` (`tests/trade_query.rs`), `edited_search_is_sent_instead_of_answered_from_the_last_one` (`tests/trade_search.rs`); each fails when the code it covers is broken. The trade site link test went with the button. Rate limits for Search are the existing `TradeSource` tests: Search goes through the same call.
- **Checked live** (stand-in game, real trade site, clicks through a virtual pointer): Death Nails opened wide with 4 stats and "No online listings like it"; unticking total life sent nothing; Search greyed out, then showed "1 listed online · 10 chaos"; Carcass Jack (unique) opened narrow; Honour Leash listed its implicit/crafted/enchant Modifiers under Search; the game window kept keyboard focus throughout.
- **After review:** old Listings are hidden while a search runs; the last item's search is dropped when a new item opens; query and league travel as one value; one grid-clearing helper. Kept by choice (user): the wide Panel on the left half still starts at x = 420 and ends ~100 px past the middle of a 1920 px screen; Search sits right under the stats, above the not-searched list (as mockup C). Not fixed: if the search thread panics, Search stays grey until the next press.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Rare in game | Alt+D over a rare opens the wide Panel with its stats and listings | |
| Untick and Search | Unticking a stat and pressing Search shows a different count or listings; the game still takes keys right after | |
| Left half | With the pointer on the right half, the wide Panel doesn't cover what you need | |
