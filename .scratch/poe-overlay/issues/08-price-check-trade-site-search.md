# 08: Price Check for everything else: open search on the trade site

**What to build:** For items poe.ninja can't price (rares, magic items, bases, etc.), the Price Check panel offers "Open on trade site", which opens the user's browser on the official trade site with a search prefilled from the item's base and mods. The overlay itself does not call the trade site: stat IDs come from a bundled snapshot (the live stats endpoint is behind Cloudflare without a browser User-Agent). Mods the snapshot can't map are listed in the panel as excluded rather than silently dropped.

**Blocked by:** 07 (Price Check for poe.ninja-priced items)

**Status:** ready-for-human

- [ ] Price Checking a rare shows the button; clicking it opens the default browser on a trade search matching the item's base and mods (button shown live; the click needs a human)
- [x] No request to the trade site is made by the overlay (true for this ticket; ticket 10 then searches inline)
- [x] Unmapped mods are visibly listed as excluded
- [x] Query building is covered by tests using real rare item samples

## Comments

### Implemented (agent, 2026-11-16)

- **Item text** (`poe1::item`): rare, magic and normal items now carry their base line and modifiers, each with its source (implicit, explicit, crafted, fractured, enchant). Both the plain and the advanced (Ctrl+Alt+C) text are read: `{ … Modifier … }` headers name the source, value ranges like `23(19-24)` and reminder lines `(…)` are dropped, and marker sections (Corrupted, Split, influences, Note…) are skipped.
- **Search** (`poe1::trade`), decided with the user:
  - Base type: the longest base type from the snapshot named in whole words in the base line, so magic names, Vestigial/Synthesised/Superior prefixes and unidentified rares all work.
  - Pseudo totals over every modifier: life (flat life plus half the strength, only with flat life, as the trade site counts it), total elemental resistance, chaos resistance, strength, dexterity, intelligence. Total elemental rather than each element, like Awakened PoE Trade: the elements are worth about the same to buyers.
  - The other explicit and fractured modifiers one by one. Weapons take their "(Local)" stats except defences; armour only its defence ones. "reduced"/"less" fall back to negative "increased"/"more" (no minimum). "Adds X to Y" is the average; the same stat on two modifiers is summed. Item rarity is a pseudo stat on the trade site and is searched as one.
  - Minimums: 90% of the value, rounded down to the value's decimals.
  - Online sellers, rarity "any non-unique", cheapest first; the league prices come from.
- **Snapshot** `data/trade/` (stats.tsv: explicit and pseudo stats without options; bases.txt; `fetched` date), made by `data/trade/update.sh` (curl with a browser User-Agent + jq).
- **Panel:** "Open on trade site" (pointer-only), the modifiers left out, and a status line. The empty "chaos divine" table header no longer shows above errors.
- **Tests:** 6 new item-text tests and `tests/trade_query.rs` (14) with 9 new Sidekick samples (MIT, commit 6acfa09, credited). Each new test was checked to fail when the code it covers is broken.
- **Checked live:** the Panel showed Death Nails with the button and "Grants Level 20 Made-up Skill" as not searched, then Havoc Ward. The trade API accepted the JSON built for Havoc Ward and Honour Leash (`error: null`; both found 0 listings, being good items).

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Button | Clicking **Open on trade site** on a rare opens your browser on a search for its base with the totals and modifiers above | |
