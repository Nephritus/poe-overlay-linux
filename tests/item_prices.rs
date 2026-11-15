//! Price Check prices from poe.ninja, through a fake fetcher. Responses
//! were recorded from poe.ninja on 2026-11-14/15 and trimmed to the items
//! used here.

use std::cell::RefCell;
use std::time::Instant;

use poe_overlay::poe1::item::{Item, ItemKind};
use poe_overlay::poe1::prices::{Fetch, ItemPrices, PriceSource};

/// Answers from recorded responses and logs every requested URL; a category
/// without a recording is empty, as poe.ninja answers for one it doesn't list.
#[derive(Default)]
struct FakeNinja {
    requests: RefCell<Vec<String>>,
}

impl Fetch for &FakeNinja {
    fn get(&self, url: &str) -> Result<String, String> {
        self.requests.borrow_mut().push(url.to_owned());
        if url.ends_with("/economy/leagues") {
            return Ok(include_str!("data/ninja-leagues.json").into());
        }
        let category = url.rsplit_once("type=").expect("a type").1;
        let exchange = url.contains("/exchange/");
        Ok(match (exchange, category) {
            (true, "Currency") => include_str!("data/ninja-exchange-currency-covens.json"),
            (true, "DivinationCard") => include_str!("data/ninja-exchange-divinationcard-covens.json"),
            (true, "Oil") => include_str!("data/ninja-exchange-oil-covens.json"),
            (true, _) => include_str!("data/ninja-exchange-scarab-covens.json"),
            (false, "SkillGem") => include_str!("data/ninja-stash-skillgem-covens.json"),
            (false, "ImbuedGem") => include_str!("data/ninja-stash-imbuedgem-covens.json"),
            (false, "UniqueArmour") => include_str!("data/ninja-stash-uniquearmour-covens.json"),
            (false, "UniqueWeapon") => include_str!("data/ninja-stash-uniqueweapon-covens.json"),
            (false, _) => r#"{"lines":[]}"#,
        }
        .into())
    }
}

impl FakeNinja {
    fn categories(&self) -> Vec<String> {
        self.requests.borrow().iter().filter_map(|url| Some(url.rsplit_once("type=")?.1.to_owned())).collect()
    }
}

fn item(class: &str, name: &str, kind: ItemKind) -> Item {
    Item { class: class.into(), name: name.into(), kind }
}

fn price(item: &Item) -> (ItemPrices, Vec<String>) {
    let ninja = FakeNinja::default();
    let prices = PriceSource::new(&ninja, None).item_prices(item, Instant::now()).unwrap();
    (prices, ninja.categories())
}

/// (label, chaos) per row.
fn rows(prices: &ItemPrices) -> Vec<(Option<&str>, f64)> {
    prices.prices.iter().map(|p| (p.label.as_deref(), p.chaos)).collect()
}

#[test]
fn currency_is_priced_per_unit_with_its_stack() {
    let (prices, _) = price(&item("Stackable Currency", "Chaos Orb", ItemKind::Currency { stack: 7 }));

    assert_eq!(rows(&prices), [(None, 1.0)]);
    assert_eq!(prices.stack, 7);
    assert_eq!(prices.league, "Covens");
}

#[test]
fn oils_are_looked_up_among_oils_first() {
    let (prices, categories) = price(&item("Stackable Currency", "Clear Oil", ItemKind::Currency { stack: 5 }));

    assert_eq!(rows(&prices), [(None, 0.05201)]);
    assert_eq!(categories, ["Oil"]);
}

#[test]
fn unknown_currency_searches_every_exchange_category_then_says_so() {
    let ninja = FakeNinja::default();
    let scarab = item("Map Fragments", "Trarthan Scarab of Infamy", ItemKind::Currency { stack: 1 });

    let err = PriceSource::new(&ninja, None).item_prices(&scarab, Instant::now()).unwrap_err();

    assert!(err.contains("Trarthan Scarab of Infamy") && err.contains("Covens"), "{err}");
    let categories = ninja.categories();
    assert_eq!(categories[0], "Scarab", "the name says scarab");
    assert!(categories.contains(&"Currency".to_owned()) && categories.contains(&"Fragment".to_owned()), "{categories:?}");
}

#[test]
fn divination_card_is_priced_in_chaos_and_divine() {
    let (prices, categories) = price(&item("Divination Cards", "The Doctor", ItemKind::DivinationCard { stack: 2 }));

    assert_eq!(rows(&prices), [(None, 1193.0)]);
    assert!((prices.prices[0].divine - 1193.0 * 0.01015).abs() < 1e-6);
    assert_eq!(categories, ["DivinationCard"]);
}

fn gem(name: &str, level: u32, quality: u32, corrupted: bool) -> Item {
    item("Skill Gems", name, ItemKind::Gem { level, quality, corrupted, imbued: None })
}

#[test]
fn gem_with_an_exact_level_quality_and_corruption_match_shows_that_price() {
    let (prices, _) = price(&gem("Kinetic Blast of Clustering", 20, 20, false));

    assert_eq!(rows(&prices).iter().map(|r| r.1).collect::<Vec<_>>(), [150.0]);
    assert!(!prices.approximate);
}

#[test]
fn gem_without_a_match_shows_the_closest_variants_labelled() {
    // Level 4, no quality: poe.ninja lists 1, 1/20, 20, 20/20 and corrupted ones.
    let (prices, _) = price(&gem("Kinetic Blast of Clustering", 4, 0, false));

    assert!(prices.approximate);
    let chaos: Vec<f64> = prices.prices.iter().map(|p| p.chaos).collect();
    assert_eq!(chaos, [5.0, 7.0, 50.0], "level 1, level 1 20% and level 20, none corrupted");
    assert!(prices.prices.iter().all(|p| p.label.as_deref().is_some_and(|l| l.starts_with("Level "))));
}

#[test]
fn vaal_gem_matches_its_corrupted_entry() {
    let (prices, _) = price(&gem("Vaal Double Strike", 1, 0, true));

    assert_eq!(rows(&prices).iter().map(|r| r.1).collect::<Vec<_>>(), [1.0]);
    assert!(!prices.approximate);
}

#[test]
fn imbued_gem_is_priced_with_its_support() {
    let imbued = item("Skill Gems", "Power Siphon", ItemKind::Gem { level: 20, quality: 0, corrupted: true, imbued: Some("Chain".into()) });

    let (prices, categories) = price(&imbued);

    assert_eq!(rows(&prices).iter().map(|r| r.1).collect::<Vec<_>>(), [197.1]);
    assert_eq!(categories, ["ImbuedGem"]);
}

fn unique(class: &str, name: &str, links: u32) -> Item {
    item(class, name, ItemKind::Unique { links })
}

#[test]
fn unique_price_depends_on_its_links() {
    for (links, chaos) in [(6, 391.3), (5, 40.0), (4, 1.0), (0, 1.0)] {
        let (prices, categories) = price(&unique("Body Armours", "Carcass Jack", links));
        assert_eq!(rows(&prices).iter().map(|r| r.1).collect::<Vec<_>>(), [chaos], "{links} links");
        assert_eq!(categories, ["UniqueArmour"], "body armours are armours");
    }
}

#[test]
fn unique_of_an_unexpected_class_is_found_in_the_other_unique_categories() {
    let (prices, categories) = price(&unique("Rings", "Wings of Entropy", 0));

    assert_eq!(rows(&prices).iter().map(|r| r.1).collect::<Vec<_>>(), [1.0]);
    assert_eq!(categories[0], "UniqueAccessory");
    assert!(categories.contains(&"UniqueWeapon".to_owned()));
}

#[test]
fn foil_price_is_not_a_normal_uniques_price() {
    let ninja = FakeNinja::default();
    let brace = unique("Gloves", "The Celestial Brace", 0);

    assert!(PriceSource::new(&ninja, None).item_prices(&brace, Instant::now()).is_err());
}

#[test]
fn unidentified_unique_lists_the_uniques_of_its_base() {
    let hatchet = item("One Hand Axes", "Jade Hatchet", ItemKind::UnidentifiedUnique);

    let (prices, _) = price(&hatchet);

    assert!(prices.approximate);
    assert_eq!(rows(&prices), [(Some("The Screaming Eagle"), 1.0)]);
}

#[test]
fn rare_items_get_a_reason_instead_of_a_price() {
    let ninja = FakeNinja::default();
    let rare = item("Wands", "Miracle Chant", ItemKind::Other { rarity: "Rare".into() });

    let err = PriceSource::new(&ninja, None).item_prices(&rare, Instant::now()).unwrap_err();

    assert!(err.contains("uniques, currency, gems and divination cards"), "{err}");
    assert!(ninja.categories().is_empty(), "nothing to ask poe.ninja");
}

#[test]
fn categories_are_fetched_once_and_shared_with_currency_rates() {
    let ninja = FakeNinja::default();
    let mut source = PriceSource::new(&ninja, None);
    let now = Instant::now();

    source.currency_rates(now).unwrap();
    source.item_prices(&item("Stackable Currency", "Chaos Orb", ItemKind::Currency { stack: 1 }), now).unwrap();
    source.item_prices(&gem("Arcane Surge Support", 1, 0, false), now).unwrap();
    source.item_prices(&gem("Arcane Surge Support", 20, 20, false), now).unwrap();

    assert_eq!(ninja.categories(), ["Currency", "SkillGem"]);
}

#[test]
fn corruption_tells_gem_prices_apart() {
    let (exact, _) = price(&gem("Kinetic Blast of Clustering", 20, 20, true));
    assert_eq!(exact.prices.iter().map(|p| p.chaos).collect::<Vec<_>>(), [140.0], "20/20 corrupted, not 20/20");

    // Level 4 corrupted: the corrupted entries come before closer uncorrupted ones.
    let (closest, _) = price(&gem("Kinetic Blast of Clustering", 4, 0, true));
    assert_eq!(closest.prices.iter().map(|p| p.chaos).collect::<Vec<_>>(), [140.0, 195.7, 195.7]);
}

#[test]
fn cheap_entries_without_a_divine_value_get_one_from_the_divine_rate() {
    // poe.ninja leaves out divineValue for items under about half a chaos.
    let (prices, categories) = price(&gem("Vaal Spectral Throw", 1, 0, true));

    assert_eq!(prices.prices[0].chaos, 0.39);
    // The currency overview recorded for these tests has 0.01071 divine per chaos.
    assert!((prices.prices[0].divine - 0.39 * 0.01071).abs() < 1e-9, "{}", prices.prices[0].divine);
    assert_eq!(categories, ["SkillGem", "Currency"]);
}

#[test]
fn items_are_priced_in_the_changed_league() {
    let ninja = FakeNinja::default();
    let mut source = PriceSource::new(&ninja, None);
    let chaos = item("Stackable Currency", "Chaos Orb", ItemKind::Currency { stack: 1 });
    source.item_prices(&chaos, Instant::now()).unwrap();

    source.set_league(Some("Standard".into()));

    assert_eq!(source.item_prices(&chaos, Instant::now()).unwrap().league, "Standard");
    assert!(ninja.requests.borrow().last().unwrap().contains("league=Standard&"));
}
