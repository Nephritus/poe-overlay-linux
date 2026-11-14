//! Item text as the game copies it (Ctrl+Alt+C), from real samples; see
//! tests/data/items/SOURCE.md.

use poe_overlay::poe1::item::{Item, ItemKind, ItemTextError, parse};

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

#[test]
fn rare_items_are_other() {
    let wand = item("rare-wand");

    assert_eq!((wand.class.as_str(), wand.kind), ("Wands", ItemKind::Other { rarity: "Rare".into() }));
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
