//! Trade site searches built from real item texts (tests/data/items) and the
//! bundled trade data (data/trade). Minimums are 90% of the item's value,
//! rounded down; ids are the snapshot's.

use poe_overlay::poe1::item::{Item, parse};
use poe_overlay::poe1::trade::{ListingKind, Rarity, TradeData, TradeQuery, bound};

fn sample(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/data/items/{name}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn item(name: &str) -> Item {
    parse(&sample(name)).unwrap()
}

fn query(item: &Item) -> TradeQuery<'static> {
    TradeQuery::new(item, TradeData::bundled()).expect("an item priced by trade search")
}

/// (stat id, minimum) per filter, in order.
fn filters(query: &TradeQuery) -> Vec<(String, Option<f64>)> {
    query.filters.iter().map(|filter| (filter.id.to_owned(), filter.min)).collect()
}

fn expected(filters: &[(&str, Option<f64>)]) -> Vec<(String, Option<f64>)> {
    filters.iter().map(|(id, min)| ((*id).to_owned(), *min)).collect()
}

#[test]
fn life_resistances_and_attributes_are_searched_as_totals() {
    let gloves = query(&item("rare-gloves-death-nails"));

    assert_eq!(gloves.base_type, Some("Assassin's Mitts"));
    assert_eq!(
        filters(&gloves),
        expected(&[
            ("pseudo.pseudo_total_life", Some(65.0)),
            ("pseudo.pseudo_total_elemental_resistance", Some(12.0)),
            ("pseudo.pseudo_total_intelligence", Some(16.0)),
            // 0.23 at 90% is 0.207, rounded down to the value's two decimals.
            ("explicit.stat_3237948413", Some(0.2)),
        ])
    );
    assert!(gloves.excluded.is_empty(), "{:?}", gloves.excluded);
}

#[test]
fn totals_count_every_source_but_enchants_and_strength_adds_half_to_life() {
    let belt = query(&item("rare-belt-honour-leash"));

    assert_eq!(belt.base_type, Some("Cord Belt"));
    assert_eq!(
        filters(&belt),
        expected(&[
            // 136 life + 25 strength / 2 = 148.5.
            ("pseudo.pseudo_total_life", Some(133.0)),
            ("pseudo.pseudo_total_elemental_resistance", Some(50.0)),
            // Fractured.
            ("pseudo.pseudo_total_chaos_resistance", Some(29.0)),
            ("pseudo.pseudo_total_strength", Some(22.0)),
            ("explicit.stat_1050105434", Some(59.0)),
        ])
    );
}

#[test]
fn all_attributes_count_toward_each_and_life_needs_flat_life() {
    let ring = query(&item("rare-ring-brood-circle"));

    assert_eq!(ring.base_type, Some("Ruby Ring"));
    assert_eq!(
        filters(&ring),
        expected(&[
            ("pseudo.pseudo_total_strength", Some(14.0)),
            ("pseudo.pseudo_total_dexterity", Some(14.0)),
            ("pseudo.pseudo_total_intelligence", Some(42.0)),
            // Adds 8 to 13: the average, 10.5.
            ("explicit.stat_3032590688", Some(9.0)),
            ("explicit.stat_789117908", Some(27.0)),
        ])
    );
}

#[test]
fn same_stat_on_two_modifiers_is_summed_and_armour_takes_its_local_stats() {
    let helmet = query(&item("rare-helmet-havoc-ward"));

    // "Vestigial Secutor Helm" trades as a Secutor Helm.
    assert_eq!(helmet.base_type, Some("Secutor Helm"));
    assert_eq!(
        filters(&helmet),
        expected(&[
            ("pseudo.pseudo_total_elemental_resistance", Some(21.0)),
            // 23% + 23% rarity: the trade site has it as a pseudo stat only.
            ("pseudo.pseudo_increased_rarity", Some(41.0)),
            // 35% + 31%, local to the helmet.
            ("explicit.stat_2451402625", Some(59.0)),
            ("explicit.stat_2511217560", Some(10.0)),
            // Accuracy on a helmet is global.
            ("explicit.stat_803737631", Some(207.0)),
        ])
    );
}

#[test]
fn weapons_take_their_local_stats() {
    let wand = query(&item("rare-wand"));

    assert_eq!(wand.base_type, Some("Imbued Wand"));
    assert_eq!(
        filters(&wand),
        expected(&[
            ("explicit.stat_1940865751", Some(11.0)),
            ("explicit.stat_3962278098", Some(21.0)),
            ("explicit.stat_2387539034", Some(9.0)),
        ])
    );
}

#[test]
fn magic_item_trades_as_the_base_inside_its_name() {
    let axe = query(&item("magic-frosted-vaal-axe"));

    assert_eq!(axe.base_type, Some("Vaal Axe"));
    assert_eq!(filters(&axe), expected(&[("explicit.stat_1037193709", Some(4.0)), ("explicit.stat_821021828", Some(21.0))]));
    assert_eq!(query(&item("magic-axe-of-the-boxer")).base_type, Some("Shadow Axe"));
}

#[test]
fn reduced_is_searched_as_negative_increased_without_a_minimum() {
    let text = sample("rare-ring-brood-circle").replace("31% increased Mana Regeneration Rate", "12% reduced Mana Regeneration Rate");

    let ring = query(&parse(&text).unwrap());

    assert_eq!(filters(&ring).last(), Some(&("explicit.stat_789117908".to_owned(), None)));
}

#[test]
fn modifiers_without_a_trade_stat_are_excluded_by_name() {
    let text = sample("rare-gloves-death-nails").replace("+18 to Intelligence", "Grants Level 20 Made-up Skill");

    let gloves = query(&parse(&text).unwrap());

    assert_eq!(gloves.excluded, ["Grants Level 20 Made-up Skill"]);
    assert!(!filters(&gloves).iter().any(|(id, _)| id == "pseudo.pseudo_total_intelligence"));
}

#[test]
fn implicit_crafted_and_enchant_modifiers_outside_totals_are_named_as_skipped() {
    let belt = query(&item("rare-belt-honour-leash"));

    assert_eq!(belt.skipped, ["Allocates As The Mountain", "Can be Anointed", "17% increased Damage"]);
    assert!(belt.excluded.is_empty());
}

#[test]
fn stat_on_two_lines_is_matched_whole() {
    let text = sample("rare-gloves-death-nails").replace(
        "+18 to Intelligence",
        "Grants Immunity to Bleeding for 4 seconds if used while Bleeding\nGrants Immunity to Corrupted Blood for 4 seconds if used while affected by Corrupted Blood",
    );

    let gloves = query(&parse(&text).unwrap());

    // explicit.stat_182714578 is the same text with `#` for the seconds.
    assert!(filters(&gloves).contains(&("explicit.stat_3233646242".to_owned(), None)), "{:?}", filters(&gloves));
    assert!(gloves.excluded.is_empty(), "{:?}", gloves.excluded);
}

#[test]
fn filters_carry_the_trade_site_wording() {
    let text = sample("rare-gloves-death-nails").replace(
        "+18 to Intelligence",
        "Grants Immunity to Bleeding for 4 seconds if used while Bleeding\nGrants Immunity to Corrupted Blood for 4 seconds if used while affected by Corrupted Blood",
    );

    let gloves = query(&parse(&text).unwrap());

    let wording: Vec<&str> = gloves.filters.iter().map(|filter| filter.text).collect();
    assert_eq!(
        wording,
        [
            "+# total maximum Life",
            "+#% total Elemental Resistance",
            "Grants Immunity to Bleeding for 4 seconds if used while Bleeding\nGrants Immunity to Corrupted Blood for 4 seconds if used while affected by Corrupted Blood",
            "#% of Physical Attack Damage Leeched as Mana",
        ]
    );
}

#[test]
fn numbers_in_a_stat_wording_tell_stats_apart() {
    for (seconds, id) in [("4", "explicit.stat_3023957681"), ("10", "explicit.stat_2453026567")] {
        let line = format!("20% chance to gain Onslaught for {seconds} seconds on Kill");
        let text = sample("rare-gloves-death-nails").replace("0.23% of Physical Attack Damage Leeched as Mana", &line);

        let gloves = query(&parse(&text).unwrap());

        assert_eq!(filters(&gloves).last(), Some(&(id.to_owned(), Some(18.0))), "{seconds} seconds");
    }
    // `Projectiles Pierce # additional Targets` reads the same; the wording one is the stat.
    let text = sample("rare-gloves-death-nails").replace("0.23% of Physical Attack Damage Leeched as Mana", "Projectiles Pierce 2 additional Targets");
    assert_eq!(filters(&query(&parse(&text).unwrap())).last(), Some(&("explicit.stat_3640956958".to_owned(), None)));
}

#[test]
fn items_without_modifiers_are_searched_by_base() {
    for (sample, base) in [("rare-mace-unidentified", "Ornate Mace"), ("normal-sorcerer-boots", "Sorcerer Boots")] {
        let query = query(&item(sample));
        assert_eq!((query.base_type, query.filters.len()), (Some(base), 0), "{sample}");
    }
}

#[test]
fn resistances_to_several_elements_count_for_each() {
    let text = sample("rare-gloves-death-nails").replace("+14% to Lightning Resistance", "+12% to all Elemental Resistances\n+10% to Fire and Cold Resistances");

    let gloves = query(&parse(&text).unwrap());

    // 3 × 12 + 2 × 10 = 56.
    assert!(filters(&gloves).contains(&("pseudo.pseudo_total_elemental_resistance".to_owned(), Some(50.0))), "{:?}", filters(&gloves));
}

#[test]
fn base_type_must_be_named_in_whole_words() {
    // "Arc" is a base type (the gem), but not these boots'.
    let text = sample("normal-sorcerer-boots").replace("Sorcerer Boots", "Arcing Boot-Shaped SuperArc");

    assert_eq!(query(&parse(&text).unwrap()).base_type, None);
}

#[test]
fn items_poe_ninja_prices_are_not_searched() {
    assert!(TradeQuery::new(&item("unique-carcass-jack-6l"), TradeData::bundled()).is_none());
}

#[test]
fn first_search_asks_for_cheapest_non_unique_listings_of_both_kinds() {
    let gloves = query(&item("rare-gloves-death-nails"));

    let json: serde_json::Value = serde_json::from_str(&gloves.to_json()).unwrap();

    // `available`: instant buyout, and in person from sellers online now.
    let expected = serde_json::json!({
        "query": {
            "status": { "option": "available" },
            "type": "Assassin's Mitts",
            "stats": [{ "type": "and", "filters": [
                { "id": "pseudo.pseudo_total_life", "value": { "min": 65.0 } },
                { "id": "pseudo.pseudo_total_elemental_resistance", "value": { "min": 12.0 } },
                { "id": "pseudo.pseudo_total_intelligence", "value": { "min": 16.0 } },
                { "id": "explicit.stat_3237948413", "value": { "min": 0.2 } },
            ]}],
            "filters": { "type_filters": { "filters": { "rarity": { "option": "nonunique" } } } },
        },
        "sort": { "price": "asc" },
    });
    assert_eq!(json, expected);
}

#[test]
fn sale_type_picks_instant_buyout_or_in_person_listings() {
    let mut gloves = query(&item("rare-gloves-death-nails"));
    let status = |gloves: &TradeQuery| serde_json::from_str::<serde_json::Value>(&gloves.to_json()).unwrap()["query"]["status"]["option"].clone();

    gloves.sale = Some(ListingKind::InstantBuyout);
    assert_eq!(status(&gloves), "securable");

    gloves.sale = Some(ListingKind::InPerson);
    assert_eq!(status(&gloves), "online");
}

#[test]
fn switched_off_filters_are_left_out_of_the_search() {
    let mut gloves = query(&item("rare-gloves-death-nails"));
    gloves.filters[0].enabled = false;
    gloves.filters[2].enabled = false;

    let json: serde_json::Value = serde_json::from_str(&gloves.to_json()).unwrap();

    let searched = serde_json::json!([
        { "id": "pseudo.pseudo_total_elemental_resistance", "value": { "min": 12.0 } },
        { "id": "explicit.stat_3237948413", "value": { "min": 0.2 } },
    ]);
    assert_eq!(json["query"]["stats"][0]["filters"], searched);
}

#[test]
fn typed_minimum_and_maximum_are_searched_and_an_empty_side_left_out() {
    let mut gloves = query(&item("rare-gloves-death-nails"));
    gloves.filters[0].max = Some(80.0);
    gloves.filters[1].min = None;
    gloves.filters[1].max = Some(30.0);
    gloves.filters[2].min = None;

    let json: serde_json::Value = serde_json::from_str(&gloves.to_json()).unwrap();

    let searched = serde_json::json!([
        { "id": "pseudo.pseudo_total_life", "value": { "min": 65.0, "max": 80.0 } },
        { "id": "pseudo.pseudo_total_elemental_resistance", "value": { "max": 30.0 } },
        { "id": "pseudo.pseudo_total_intelligence" },
        { "id": "explicit.stat_3237948413", "value": { "min": 0.2 } },
    ]);
    assert_eq!(json["query"]["stats"][0]["filters"], searched);
}

#[test]
fn typed_bounds_are_numbers_or_nothing() {
    assert_eq!(bound(""), Ok(None));
    assert_eq!(bound("  "), Ok(None));
    assert_eq!(bound("65"), Ok(Some(65.0)));
    assert_eq!(bound(" 0.25 "), Ok(Some(0.25)));
    assert_eq!(bound("-12"), Ok(Some(-12.0)));
    for junk in ["abc", "6 5", "1e999", "NaN", "inf", "12%"] {
        assert!(bound(junk).is_err(), "{junk}");
    }
}

/// The search as JSON, for reading single fields.
fn json(query: &TradeQuery) -> serde_json::Value {
    serde_json::from_str(&query.to_json()).unwrap()
}

/// The Death Nails gloves as another item class and base.
fn gloves_as(class: &str, base: &str) -> Item {
    parse(&sample("rare-gloves-death-nails").replace("Item Class: Gloves", &format!("Item Class: {class}")).replace("Assassin's Mitts", base)).unwrap()
}

#[test]
fn any_item_of_its_class_is_searched_by_category_instead_of_base() {
    let mut gloves = query(&item("rare-gloves-death-nails"));
    assert_eq!(gloves.class_category, Some("armour.gloves"));

    gloves.of_class = true;

    let json = json(&gloves);
    assert_eq!(json["query"]["filters"]["type_filters"]["filters"]["category"], serde_json::json!({ "option": "armour.gloves" }));
    assert!(json["query"].get("type").is_none(), "{json}");
}

#[test]
fn item_classes_map_to_the_trade_sites_own_categories() {
    for (class, base, category) in [
        ("Body Armours", "Vaal Regalia", "armour.chest"),
        ("Rune Daggers", "Platinum Kris", "weapon.runedagger"),
        ("Thrusting One Hand Swords", "Jewelled Foil", "weapon.rapier"),
        ("Staves", "Eclipse Staff", "weapon.basestaff"),
        ("Utility Flasks", "Quicksilver Flask", "flask"),
        ("Jewels", "Cobalt Jewel", "jewel.base"),
        ("Jewels", "Large Cluster Jewel", "jewel.cluster"),
    ] {
        assert_eq!(query(&gloves_as(class, base)).class_category, Some(category), "{class} {base}");
    }
}

#[test]
fn an_item_class_the_trade_site_has_no_category_for_is_searched_by_its_base() {
    let mut odd = query(&gloves_as("Made-up Things", "Assassin's Mitts"));
    assert_eq!(odd.class_category, None);

    odd.of_class = true;

    assert_eq!(json(&odd)["query"]["type"], "Assassin's Mitts");
}

#[test]
fn a_base_missing_from_the_trade_data_is_searched_by_its_class_from_the_first_search() {
    let gloves = query(&gloves_as("Gloves", "Zzyzx Qwerty"));
    assert_eq!(gloves.base_type, None);

    let json = json(&gloves);

    assert_eq!(json["query"]["filters"]["type_filters"]["filters"]["category"], serde_json::json!({ "option": "armour.gloves" }));
}

#[test]
fn rarity_choice_narrows_the_search() {
    let mut gloves = query(&item("rare-gloves-death-nails"));
    let rarity = |gloves: &TradeQuery| json(gloves)["query"]["filters"]["type_filters"]["filters"]["rarity"]["option"].clone();
    assert_eq!(rarity(&gloves), "nonunique");

    for (choice, option) in [(Rarity::Rare, "rare"), (Rarity::Magic, "magic"), (Rarity::Normal, "normal")] {
        gloves.rarity = Some(choice);
        assert_eq!(rarity(&gloves), option);
    }
}
