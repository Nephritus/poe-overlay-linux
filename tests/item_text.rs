//! Item text as the game copies it (Ctrl+Alt+C), from real samples; see
//! tests/data/items/SOURCE.md.

use poe_overlay::poe1::item::{Item, ItemKind, ItemTextError, ModifierSource, parse};

fn sample(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/data/items/{name}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn item(name: &str) -> Item {
    parse(&sample(name)).unwrap_or_else(|err| panic!("{name}: {err}"))
}

#[test]
fn currency_has_its_name_and_stack_size() {
    let chaos = item("currency-chaos-orb");

    assert_eq!(chaos.name, "Chaos Orb");
    assert_eq!(chaos.kind, ItemKind::Currency { stack: 1 });
}

#[test]
fn stackables_other_than_currency_are_currency_too() {
    assert_eq!(item("currency-clear-oil").kind, ItemKind::Currency { stack: 5 });
    let scarab = item("fragment-trarthan-scarab");
    assert_eq!((scarab.name.as_str(), scarab.kind), ("Trarthan Scarab of Infamy", ItemKind::Currency { stack: 1 }));
}

#[test]
fn divination_card_has_its_stack_size() {
    let card = item("card-the-saints-treasure");

    assert_eq!((card.name.as_str(), card.kind), ("The Saint's Treasure", ItemKind::DivinationCard { stack: 1 }));
}

#[test]
fn gems_have_level_quality_and_corruption() {
    for (sample, name, kind) in [
        ("gem-arcane-surge-support", "Arcane Surge Support", ItemKind::Gem { level: 1, quality: 0, corrupted: false, imbued: None }),
        ("gem-kinetic-blast-of-clustering", "Kinetic Blast of Clustering", ItemKind::Gem { level: 4, quality: 0, corrupted: false, imbued: None }),
        // Imbued gems trade with their built-in support.
        ("gem-earthshatter-imbued", "Earthshatter", ItemKind::Gem { level: 20, quality: 20, corrupted: true, imbued: Some("Knockback".into()) }),
        // Trades under its Vaal name.
        ("gem-vaal-double-strike", "Vaal Double Strike", ItemKind::Gem { level: 1, quality: 0, corrupted: true, imbued: None }),
    ] {
        let gem = item(sample);
        assert_eq!((gem.name.as_str(), gem.kind), (name, kind), "{sample}");
    }
}

#[test]
fn uniques_have_name_and_largest_link_group() {
    let jack = item("unique-carcass-jack-6l");
    assert_eq!(jack.name, "Carcass Jack");
    assert_eq!(jack.kind, ItemKind::Unique { links: 6 });

    let wings = item("unique-wings-of-entropy");
    assert_eq!(wings.kind, ItemKind::Unique { links: 3 });
}

#[test]
fn unidentified_unique_is_known_by_its_base_only() {
    let hatchet = item("unique-unidentified-jade-hatchet");

    assert_eq!((hatchet.name.as_str(), hatchet.kind), ("Jade Hatchet", ItemKind::UnidentifiedUnique));
}

/// The kind of a rare, magic or normal item, split into parts.
fn other(name: &str) -> (String, String, Vec<(ModifierSource, String)>) {
    match item(name).kind {
        ItemKind::Other { rarity, base_line, modifiers } => {
            (rarity, base_line, modifiers.into_iter().map(|modifier| (modifier.source, modifier.text)).collect())
        }
        kind => panic!("{name}: {kind:?}"),
    }
}

fn explicit(texts: &[&str]) -> Vec<(ModifierSource, String)> {
    texts.iter().map(|text| (ModifierSource::Explicit, (*text).to_owned())).collect()
}

#[test]
fn rare_has_its_name_base_line_and_modifiers() {
    let gloves = item("rare-gloves-death-nails");
    assert_eq!((gloves.class.as_str(), gloves.name.as_str()), ("Gloves", "Death Nails"));

    let (rarity, base_line, modifiers) = other("rare-gloves-death-nails");

    assert_eq!((rarity.as_str(), base_line.as_str()), ("Rare", "Assassin's Mitts"));
    let expected = ["+18 to Intelligence", "+73 to maximum Life", "+14% to Lightning Resistance", "0.23% of Physical Attack Damage Leeched as Mana"];
    assert_eq!(modifiers, explicit(&expected));
}

#[test]
fn modifiers_know_where_they_come_from() {
    let (_, _, modifiers) = other("rare-belt-honour-leash");

    let expected = [
        (ModifierSource::Enchant, "Allocates As The Mountain"),
        (ModifierSource::Implicit, "Can be Anointed"),
        (ModifierSource::Fractured, "+33% to Chaos Resistance"),
        (ModifierSource::Explicit, "+25 to Strength"),
        (ModifierSource::Explicit, "+136 to maximum Life"),
        (ModifierSource::Explicit, "+66 to maximum Mana"),
        (ModifierSource::Explicit, "+56% to Fire Resistance"),
        (ModifierSource::Crafted, "17% increased Damage"),
    ];
    assert_eq!(modifiers, expected.map(|(source, text)| (source, text.to_owned())));
}

#[test]
fn advanced_text_drops_value_ranges_headers_and_reminders() {
    let (_, base_line, modifiers) = other("rare-helmet-havoc-ward");

    assert_eq!(base_line, "Vestigial Secutor Helm");
    let mut expected = vec![(ModifierSource::Implicit, "Your Fire Damage can Shock but not Ignite".to_owned())];
    expected.extend(explicit(&[
        "23% increased Rarity of Items found",
        "35% increased Armour and Evasion",
        "31% increased Armour and Evasion",
        "12% increased Stun and Block Recovery",
        "23% increased Rarity of Items found",
        "+231 to Accuracy Rating",
        "+24% to Lightning Resistance",
    ]));
    assert_eq!(modifiers, expected);
}

#[test]
fn advanced_text_keeps_fractured_and_implicit_headers_apart() {
    let text = sample("rare-boots-invasion-track").replace(
        "10% increased Movement Speed (fractured)\n+5 to maximum Life",
        "{ Fractured Prefix Modifier \"Runner's\" (Tier: 7) — Speed }\n10(10-14)% increased Movement Speed\n{ Prefix Modifier \"Hale\" (Tier: 10) — Life }\n+5(3-9) to maximum Life",
    );
    let ItemKind::Other { modifiers, .. } = parse(&text).unwrap().kind else { panic!() };

    assert_eq!((modifiers[0].source, modifiers[0].text.as_str()), (ModifierSource::Fractured, "10% increased Movement Speed"));
    assert_eq!((modifiers[1].source, modifiers[1].text.as_str()), (ModifierSource::Explicit, "+5 to maximum Life"));
}

#[test]
fn magic_and_normal_items_have_one_name_line() {
    let (rarity, base_line, modifiers) = other("magic-frosted-vaal-axe");
    assert_eq!((rarity.as_str(), base_line.as_str()), ("Magic", "Frosted Vaal Axe of Recuperation"));
    let mut expected = vec![(ModifierSource::Implicit, "25% chance to Maim on Hit".to_owned())];
    expected.extend(explicit(&["Adds 3 to 7 Cold Damage", "Grants 24 Life per Enemy Hit"]));
    assert_eq!(modifiers, expected);

    assert_eq!(other("normal-sorcerer-boots"), ("Normal".into(), "Sorcerer Boots".into(), vec![]));
}

#[test]
fn unidentified_rare_shows_its_base_and_implicit_only() {
    let (_, base_line, modifiers) = other("rare-mace-unidentified");

    assert_eq!(base_line, "Ornate Mace");
    assert_eq!(modifiers, [(ModifierSource::Implicit, "15% reduced Enemy Stun Threshold".to_owned())]);
}

#[test]
fn game_line_endings_are_understood() {
    // The game copies Windows line endings through Wine.
    let crlf = sample("unique-carcass-jack-6l").replace('\n', "\r\n");

    assert_eq!(parse(&crlf).unwrap(), item("unique-carcass-jack-6l"));
}

#[test]
fn other_languages_and_other_text_are_told_apart() {
    assert_eq!(parse(&sample("korean-unique-belt")), Err(ItemTextError::NotEnglish));
    for text in ["", "hello", "https://www.pathofexile.com/trade", "Item Class: Wands"] {
        assert_eq!(parse(text), Err(ItemTextError::NotAnItem), "{text:?}");
    }
    assert!(ItemTextError::NotEnglish.to_string().contains("English game client only"));
}

#[test]
fn links_count_sockets_joined_by_dashes_only() {
    let text = sample("unique-carcass-jack-6l").replace("Sockets: B-B-R-B-B-B", "Sockets: B-B R-B-B G");
    assert_eq!(parse(&text).unwrap().kind, ItemKind::Unique { links: 3 });
}

#[test]
fn stacks_in_currency_tabs_have_thousands_separators() {
    let text = sample("currency-chaos-orb").replace("Stack Size: 1/10", "Stack Size: 2,448/10");

    assert_eq!(parse(&text).unwrap().kind, ItemKind::Currency { stack: 2448 });
}
