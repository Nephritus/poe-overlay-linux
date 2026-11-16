# 22: Item type and rarity filters

**What to build:** The wide Price Check Panel gets an item type drop-down (the item's own base, or any item of its class, e.g. "Any Gloves") and a rarity drop-down (any non-unique, rare, magic, normal). The first search keeps today's choice (this base, any non-unique); changes feed the next Search.

**Blocked by:** 19 (Wide Price Check Panel with switchable filters)

**Status:** ready-for-human

- [x] Choosing the item's class and pressing Search finds Listings of other bases in that class
- [x] Choosing a rarity and pressing Search shows only that rarity
- [x] The first search of a check is unchanged from today

## Comments

### Implemented (agent, 2026-11-16)

- **Controls:** two rows of linked toggle buttons above the stats, like Sale (user's choice over drop-downs): *Item* — the base, *Any \<class\>* (the class as the game names it, e.g. "Any Gloves"); *Rarity* — *Any non-unique*, *Rare*, *Magic*, *Normal*. An item class the trade site has no category for offers only its base. The first search is the base and any non-unique, as before.
- **Search:** any item of the class sends the trade site's category (`type_filters.category`, e.g. `armour.gloves`) and no base type; the rarity option is `nonunique`, `rare`, `magic` or `normal`. Categories follow the game's item classes (37, e.g. Rune Daggers → `weapon.runedagger`, Thrusting One Hand Swords → `weapon.rapier`, all four flask classes → `flask`), checked against the trade site's filter list fetched 2026-11-16 and Sidekick's table. Cluster jewels share the class "Jewels" with the rest and are told apart by base (`jewel.cluster`).
- **Tests:** `any_item_of_its_class_is_searched_by_category_instead_of_base`, `item_classes_map_to_the_trade_sites_own_categories`, `an_item_class_the_trade_site_has_no_category_for_is_searched_by_its_base`, `rarity_choice_narrows_the_search` (`tests/trade_query.rs`); each fails when the code it covers is broken.
- **Checked live:** Death Nails opened with *Assassin's Mitts*, *Any non-unique*, *Both*: "1 listed" (1 divine, instant). Picking *Any Gloves* and *Rare*, then Search: "49 listed", 1 to 4 chaos, instant and in-person rows. The game kept keyboard focus.
- **After review:** a base missing from the trade data snapshot (the class known) is searched by class from the first search, matching the only *Item* button shown (`a_base_missing_from_the_trade_data_is_searched_by_its_class_from_the_first_search`); before, the first search sent neither base nor class. Item type labels no longer copy the base name; one child-clearing helper for every Panel container. Not changed: an item with neither a known base nor a class category shows an empty *Item* row.

**Needs a check (human):**

| Check | Pass when | Result |
|---|---|---|
| Class search | On a rare body armour, *Any Body Armours* + Search lists other bases with similar stats | |
