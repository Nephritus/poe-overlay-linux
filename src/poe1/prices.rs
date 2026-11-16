//! Prices from poe.ninja's documented PoE 1 economy endpoints, cached so
//! poe.ninja is asked at most once per league and category per 15 minutes
//! (once a minute while it fails).

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::item::{Item, ItemKind};

const NINJA: &str = "https://poe.ninja/poe1/api/economy";

/// poe.ninja refreshes PoE 1 prices about every 15 minutes.
const MAX_AGE: Duration = Duration::from_secs(15 * 60);

/// How long a failed fetch is reported again before poe.ninja is retried.
const RETRY_AFTER: Duration = Duration::from_secs(60);

/// How many currencies the Currency Rates Panel lists.
const LISTED: usize = 12;

/// How many of a gem's variants are shown when none matches it exactly.
const CLOSEST_GEMS: usize = 3;

/// Currency exchange categories, in the order an unknown currency item is
/// looked for.
const EXCHANGE: [&str; 17] = [
    "Currency",
    "Fragment",
    "Scarab",
    "Essence",
    "Fossil",
    "Resonator",
    "Oil",
    "DeliriumOrb",
    "Omen",
    "Tattoo",
    "Artifact",
    "Runegraft",
    "AllflameEmber",
    "DjinnCoin",
    "Ducat",
    "EnshroudingCrystal",
    "Astrolabe",
];

/// Unique item categories, in the order a unique is looked for.
const UNIQUES: [&str; 8] =
    ["UniqueWeapon", "UniqueArmour", "UniqueAccessory", "UniqueFlask", "UniqueJewel", "UniqueMap", "UniqueTincture", "UniqueRelic"];

/// Fetches a URL's body as text. The seam between the price source and the
/// network: HTTP in the Overlay, recorded responses in tests.
pub trait Fetch {
    fn get(&self, url: &str) -> Result<String, String>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyRate {
    pub name: String,
    pub chaos: f64,
    pub divine: f64,
}

/// The most traded currencies of a league, by trade volume.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyRates {
    pub league: String,
    pub rates: Vec<CurrencyRate>,
}

/// What a Price Check found on poe.ninja for one item.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemPrices {
    pub league: String,
    /// The item's stack size, for the stack's worth; 1 if it doesn't stack.
    pub stack: u32,
    /// Per item, one per matching poe.ninja entry (a unique's variants).
    pub prices: Vec<ItemPrice>,
    /// Nothing matched exactly; `prices` are the closest entries (a gem's
    /// other levels, the uniques an unidentified item can be).
    pub approximate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemPrice {
    /// What tells this entry apart: a variant, a gem's level and quality.
    pub label: Option<String>,
    pub chaos: f64,
    pub divine: f64,
    /// Listings behind the price, where poe.ninja says.
    pub listings: Option<u32>,
}

pub struct PriceSource<F> {
    fetch: F,
    /// From config; `None` means the league poe.ninja lists first.
    league: Option<Arc<str>>,
    leagues: Option<Fetched<Arc<[Arc<str>]>>>,
    exchange: HashMap<Category, Fetched<Arc<Exchange>>>,
    stash: HashMap<Category, Fetched<Arc<[StashLine]>>>,
}

/// A league and one of poe.ninja's categories in it, e.g. `SkillGem`.
type Category = (Arc<str>, &'static str);

/// A fetch result and when it was fetched.
struct Fetched<T> {
    at: Instant,
    result: Result<T, String>,
}

impl<T: Clone> Fetched<T> {
    /// The result while it may be reused: a success for 15 minutes, a
    /// failure for a minute (so outages and typos don't hammer poe.ninja).
    fn reusable(&self, now: Instant) -> Option<Result<T, String>> {
        let max_age = if self.result.is_ok() { MAX_AGE } else { RETRY_AFTER };
        (now.duration_since(self.at) < max_age).then(|| self.result.clone())
    }
}

/// One currency exchange category.
struct Exchange {
    divine_per_chaos: f64,
    /// Chaos per item, by item name.
    chaos: HashMap<String, f64>,
    /// The most traded of the category.
    top: Arc<CurrencyRates>,
}

/// One entry of a stash item category.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StashLine {
    name: String,
    base_type: Option<String>,
    variant: Option<String>,
    links: Option<u32>,
    gem_level: Option<u32>,
    gem_quality: Option<u32>,
    corrupted: Option<bool>,
    /// 9 marks foil (relic) uniques.
    item_class: Option<u32>,
    chaos_value: f64,
    /// Left out by poe.ninja for items under about half a chaos; filled in
    /// from the divine rate when fetched.
    divine_value: Option<f64>,
    listing_count: Option<u32>,
}

impl StashLine {
    fn price(&self, label: Option<String>) -> ItemPrice {
        let divine = self.divine_value.unwrap_or_default();
        ItemPrice { label, chaos: self.chaos_value, divine, listings: self.listing_count }
    }

    fn is_foil(&self) -> bool {
        self.item_class == Some(9)
    }
}

impl<F: Fetch> PriceSource<F> {
    pub fn new(fetch: F, league: Option<String>) -> Self {
        Self { fetch, league: league.map(Arc::from), leagues: None, exchange: HashMap::new(), stash: HashMap::new() }
    }

    /// Prices in `league` from now on; `None`: the one poe.ninja lists first.
    /// Prices fetched for other leagues stay cached.
    pub fn set_league(&mut self, league: Option<String>) {
        self.league = league.map(Arc::from);
    }

    /// The leagues poe.ninja prices, in its order (the current challenge
    /// league first).
    pub fn leagues(&mut self, now: Instant) -> Result<Arc<[Arc<str>]>, String> {
        if let Some(reused) = self.leagues.as_ref().and_then(|l| l.reusable(now)) {
            return reused;
        }
        #[derive(Deserialize)]
        struct League {
            id: String,
        }
        let result = self.get(&format!("{NINJA}/leagues")).and_then(|body| {
            let leagues: Vec<League> = serde_json::from_str(&body).map_err(|err| unexpected("league list", err))?;
            Ok(leagues.into_iter().map(|league| Arc::from(league.id)).collect())
        });
        self.leagues = Some(Fetched { at: now, result: result.clone() });
        result
    }

    /// Currency Rates for the league, reusing earlier results (see
    /// [`Fetched::reusable`]). `now` is the caller's clock. Errors are
    /// sentences for the player.
    pub fn currency_rates(&mut self, now: Instant) -> Result<Arc<CurrencyRates>, String> {
        let league = self.league(now)?;
        Ok(self.exchange(&league, "Currency", now)?.top.clone())
    }

    /// Currency Rates if they can be answered without asking poe.ninja.
    pub fn cached_currency_rates(&self, now: Instant) -> Option<Result<Arc<CurrencyRates>, String>> {
        let league = match &self.league {
            Some(league) => league.clone(),
            None => match self.leagues.as_ref()?.reusable(now)? {
                Ok(leagues) => leagues.first()?.clone(),
                Err(err) => return Some(Err(err)),
            },
        };
        let cached = self.exchange.get(&(league, "Currency"))?.reusable(now)?;
        Some(cached.map(|exchange| exchange.top.clone()))
    }

    /// What `item` is worth: uniques, currency, gems and divination cards.
    pub fn item_prices(&mut self, item: &Item, now: Instant) -> Result<ItemPrices, String> {
        if let ItemKind::Other { rarity, .. } = &item.kind {
            return Err(format!(
                "Price Check prices uniques, currency, gems and divination cards from poe.ninja; this is a {} item.",
                rarity.to_lowercase()
            ));
        }
        let league = self.league(now)?;
        let found = match &item.kind {
            ItemKind::Currency { .. } => self.exchange_price(&league, item, exchange_categories(item), now)?,
            ItemKind::DivinationCard { .. } => self.exchange_price(&league, item, ["DivinationCard"], now)?,
            ItemKind::Gem { level, quality, corrupted, imbued } => {
                let category = if imbued.is_some() { "ImbuedGem" } else { "SkillGem" };
                let lines = self.stash(&league, category, now)?;
                let candidates: Vec<&StashLine> =
                    lines.iter().filter(|line| line.name == item.name && (imbued.is_none() || line.variant == *imbued)).collect();
                // Imbued gems are listed by their support alone.
                let exact = candidates.iter().find(|line| {
                    imbued.is_some()
                        || (line.gem_level == Some(*level)
                            && line.gem_quality.unwrap_or(0) == *quality
                            && line.corrupted.unwrap_or(false) == *corrupted)
                });
                match exact {
                    Some(line) => Some((vec![line.price(None)], false)),
                    None if candidates.is_empty() => None,
                    None => Some((closest_gems(candidates, *level, *quality, *corrupted), true)),
                }
            }
            ItemKind::Unique { links, .. } => {
                self.find_uniques(&league, &item.class, now, |line| line.name == item.name, |matching| unique_prices(matching, *links))?
            }
            // Any of the uniques on its base.
            ItemKind::UnidentifiedUnique => self.find_uniques(
                &league,
                &item.class,
                now,
                |line| line.base_type.as_deref() == Some(item.name.as_str()),
                |matching| (matching.iter().map(|line| line.price(Some(line.name.clone()))).collect(), true),
            )?,
            ItemKind::Other { .. } => None,
        };
        let Some((prices, approximate)) = found else {
            return Err(match item.kind {
                ItemKind::UnidentifiedUnique => format!("poe.ninja knows no unique {} in {league}.", item.name),
                _ => format!("poe.ninja has no price for {} in {league}.", item.name),
            });
        };
        let stack = match item.kind {
            ItemKind::Currency { stack } | ItemKind::DivinationCard { stack } => stack,
            _ => 1,
        };
        Ok(ItemPrices { league: league.to_string(), stack, prices, approximate })
    }

    /// The first price for `item`'s name in the exchange `categories`.
    fn exchange_price(
        &mut self,
        league: &Arc<str>,
        item: &Item,
        categories: impl IntoIterator<Item = &'static str>,
        now: Instant,
    ) -> Result<Option<(Vec<ItemPrice>, bool)>, String> {
        for category in categories {
            let exchange = self.exchange(league, category, now)?;
            if let Some(&chaos) = exchange.chaos.get(&item.name) {
                let price = ItemPrice { label: None, chaos, divine: chaos * exchange.divine_per_chaos, listings: None };
                return Ok(Some((vec![price], false)));
            }
        }
        Ok(None)
    }

    /// Looks through the unique categories, the one `class` suggests first,
    /// for non-foil entries that match; `prices` turns them into rows.
    fn find_uniques(
        &mut self,
        league: &Arc<str>,
        class: &str,
        now: Instant,
        matches: impl Fn(&StashLine) -> bool,
        prices: impl Fn(&[&StashLine]) -> (Vec<ItemPrice>, bool),
    ) -> Result<Option<(Vec<ItemPrice>, bool)>, String> {
        for category in unique_categories(class) {
            let lines = self.stash(league, category, now)?;
            let matching: Vec<&StashLine> = lines.iter().filter(|line| !line.is_foil() && matches(line)).collect();
            if !matching.is_empty() {
                return Ok(Some(prices(&matching)));
            }
        }
        Ok(None)
    }

    fn exchange(&mut self, league: &Arc<str>, category: &'static str, now: Instant) -> Result<Arc<Exchange>, String> {
        let key = (league.clone(), category);
        if let Some(reused) = self.exchange.get(&key).and_then(|cached| cached.reusable(now)) {
            return reused;
        }
        let url = format!("{NINJA}/exchange/current/overview?league={}&type={category}", encode(league));
        let configured = self.league.is_some();
        let result = self.get(&url).and_then(|body| parse_exchange(&body, league, category, configured)).map(Arc::new);
        self.exchange.insert(key, Fetched { at: now, result: result.clone() });
        result
    }

    fn stash(&mut self, league: &Arc<str>, category: &'static str, now: Instant) -> Result<Arc<[StashLine]>, String> {
        let key = (league.clone(), category);
        if let Some(reused) = self.stash.get(&key).and_then(|cached| cached.reusable(now)) {
            return reused;
        }
        #[derive(Deserialize)]
        struct Overview {
            lines: Vec<StashLine>,
        }
        let url = format!("{NINJA}/stash/current/item/overview?league={}&type={category}", encode(league));
        let parsed = self.get(&url).and_then(|body| {
            let overview: Overview = serde_json::from_str(&body).map_err(|err| unexpected("item overview", err))?;
            Ok(overview.lines)
        });
        let result = parsed.and_then(|mut lines| {
            if lines.iter().any(|line| line.divine_value.is_none()) {
                let rate = self.exchange(league, "Currency", now)?.divine_per_chaos;
                for line in lines.iter_mut().filter(|line| line.divine_value.is_none()) {
                    line.divine_value = Some(line.chaos_value * rate);
                }
            }
            Ok(Arc::from(lines))
        });
        self.stash.insert(key, Fetched { at: now, result: result.clone() });
        result
    }

    /// The league prices are in: the configured one, or poe.ninja's first.
    pub fn league(&mut self, now: Instant) -> Result<Arc<str>, String> {
        if let Some(league) = &self.league {
            return Ok(league.clone());
        }
        self.leagues(now)?.first().cloned().ok_or_else(|| "poe.ninja lists no leagues".to_owned())
    }

    fn get(&self, url: &str) -> Result<String, String> {
        self.fetch.get(url).map_err(|err| format!("Couldn't reach poe.ninja: {err}"))
    }
}

/// The exchange categories to look for a currency item in: the one its name
/// or class suggests first, then the rest.
fn exchange_categories(item: &Item) -> impl Iterator<Item = &'static str> {
    let name = item.name.as_str();
    let hint = if name.ends_with(" Oil") {
        Some("Oil")
    } else if name.contains("Scarab") {
        Some("Scarab")
    } else if name.contains("Essence") {
        Some("Essence")
    } else if name.ends_with("Fossil") {
        Some("Fossil")
    } else if name.contains("Resonator") {
        Some("Resonator")
    } else if name.contains("Delirium Orb") {
        Some("DeliriumOrb")
    } else if name.starts_with("Omen of") {
        Some("Omen")
    } else if name.contains("Tattoo") {
        Some("Tattoo")
    } else if item.class == "Map Fragments" {
        Some("Fragment")
    } else {
        None
    };
    hint.into_iter().chain(EXCHANGE.into_iter().filter(move |category| Some(*category) != hint))
}

/// The unique categories, the one an item class belongs to first.
fn unique_categories(class: &str) -> impl Iterator<Item = &'static str> {
    let first = match class {
        "Body Armours" | "Helmets" | "Gloves" | "Boots" | "Shields" | "Quivers" => "UniqueArmour",
        "Amulets" | "Rings" | "Belts" => "UniqueAccessory",
        "Jewels" | "Abyss Jewels" => "UniqueJewel",
        "Maps" => "UniqueMap",
        "Tinctures" => "UniqueTincture",
        "Relics" => "UniqueRelic",
        class if class.ends_with("Flasks") => "UniqueFlask",
        _ => "UniqueWeapon",
    };
    std::iter::once(first).chain(UNIQUES.into_iter().filter(move |category| *category != first))
}

/// A unique's entries for its links: 5 and 6 links are priced on their own,
/// fewer as one. Variants become labelled rows.
fn unique_prices(matching: &[&StashLine], links: u32) -> (Vec<ItemPrice>, bool) {
    let wanted = |line: &&&StashLine| match links {
        5 | 6 => line.links == Some(links),
        _ => line.links.unwrap_or(0) < 5,
    };
    let rows: Vec<ItemPrice> = matching.iter().filter(wanted).map(|line| line.price(line.variant.clone())).collect();
    if !rows.is_empty() {
        return (rows, false);
    }
    // No entry for these links: show the others, saying which they are.
    let rows = matching
        .iter()
        .map(|line| {
            let links = match line.links {
                Some(links @ 5..) => format!("{links} links"),
                _ => "under 5 links".into(),
            };
            let label = match &line.variant {
                Some(variant) => format!("{variant}, {links}"),
                None => links,
            };
            line.price(Some(label))
        })
        .collect();
    (rows, true)
}

/// The gem's entries closest to its level, quality and corruption: the same
/// corruption first, then the nearest level, then the nearest quality.
fn closest_gems(mut candidates: Vec<&StashLine>, level: u32, quality: u32, corrupted: bool) -> Vec<ItemPrice> {
    candidates.sort_by_key(|line| {
        (
            line.corrupted.unwrap_or(false) != corrupted,
            line.gem_level.unwrap_or(1).abs_diff(level),
            line.gem_quality.unwrap_or(0).abs_diff(quality),
        )
    });
    candidates
        .into_iter()
        .take(CLOSEST_GEMS)
        .map(|line| {
            let mut label = format!("Level {}", line.gem_level.unwrap_or(1));
            if let Some(quality) = line.gem_quality.filter(|q| *q > 0) {
                let _ = write!(label, ", {quality}% quality");
            }
            if line.corrupted == Some(true) {
                label.push_str(", corrupted");
            }
            line.price(Some(label))
        })
        .collect()
}

fn unexpected(what: &str, err: impl std::fmt::Display) -> String {
    format!("poe.ninja sent an unexpected {what} ({err})")
}

/// `configured`: the league came from config.toml, so name it in errors.
fn parse_exchange(body: &str, league: &str, category: &str, configured: bool) -> Result<Exchange, String> {
    #[derive(Deserialize)]
    struct Overview {
        core: Core,
        lines: Vec<Line>,
        items: Vec<Name>,
    }
    #[derive(Deserialize)]
    struct Core {
        /// Units of each secondary currency one primary (chaos) buys.
        rates: HashMap<String, f64>,
        primary: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Line {
        id: String,
        /// Price in the primary currency; left out for untraded entries.
        primary_value: Option<f64>,
        #[serde(default)]
        volume_primary_value: f64,
    }
    #[derive(Deserialize)]
    struct Name {
        id: String,
        name: String,
    }
    let overview: Overview = serde_json::from_str(body).map_err(|err| unexpected("currency overview", err))?;
    // Every league trades currency; other categories may be empty.
    if overview.lines.is_empty() && category == "Currency" {
        let hint = if configured { "; check [prices] league in config.toml" } else { "" };
        return Err(format!("poe.ninja has no prices for the league \"{league}\"{hint}"));
    }
    if overview.core.primary != "chaos" {
        return Err(unexpected("currency overview", format!("prices in {}", overview.core.primary)));
    }
    // A league with next to no trading (e.g. permanent Hardcore) has no divine rate.
    let Some(&divine_per_chaos) = overview.core.rates.get("divine") else {
        return Err(format!("poe.ninja has too few trades in {league} to price {}.", category.to_lowercase()));
    };
    let names: HashMap<&str, &str> = overview.items.iter().map(|i| (i.id.as_str(), i.name.as_str())).collect();
    let chaos = overview
        .lines
        .iter()
        .filter_map(|line| Some(((*names.get(line.id.as_str())?).to_owned(), line.primary_value?)))
        .collect();
    let mut lines: Vec<&Line> = overview.lines.iter().filter(|l| l.id != "chaos").collect();
    lines.sort_by(|a, b| b.volume_primary_value.total_cmp(&a.volume_primary_value));
    let rates = lines
        .into_iter()
        .filter_map(|line| {
            let chaos = line.primary_value?;
            Some(CurrencyRate { name: (*names.get(line.id.as_str())?).to_owned(), chaos, divine: chaos * divine_per_chaos })
        })
        .take(LISTED)
        .collect();
    let top = Arc::new(CurrencyRates { league: league.to_owned(), rates });
    Ok(Exchange { divine_per_chaos, chaos, top })
}

/// Percent-encodes text for a URL path or query (all but unreserved bytes).
pub(crate) fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}
