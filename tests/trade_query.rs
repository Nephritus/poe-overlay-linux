//! Trade site searches built from real item texts (tests/data/items) and the
//! bundled trade data (data/trade). Minimums are 90% of the item's value,
//! rounded down; ids are the snapshot's.

use poe_overlay::poe1::item::{Item, parse};
use poe_overlay::poe1::trade::{TradeData, TradeQuery};

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
fn search_asks_for_cheapest_online_non_unique_listings() {
    let gloves = query(&item("rare-gloves-death-nails"));

    let json: serde_json::Value = serde_json::from_str(&gloves.to_json()).unwrap();

    let expected = serde_json::json!({
        "query": {
            "status": { "option": "online" },
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
fn trade_site_link_carries_the_league_and_search() {
    let gloves = query(&item("rare-gloves-death-nails"));

    let url = gloves.site_url("Hardcore Covens");

    let search = url.strip_prefix("https://www.pathofexile.com/trade/search/Hardcore%20Covens?q=").expect(&url);
    assert!(!search.contains([' ', '"', '{', '\'']), "{search}");
    assert_eq!(percent_decode(search), gloves.to_json());
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            out.push(u8::from_str_radix(&text[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}
