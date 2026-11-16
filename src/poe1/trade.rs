//! Trade site searches for the items poe.ninja doesn't price (rare, magic and
//! normal items): the base type, the life, resistance and attribute totals,
//! and the other explicit modifiers, each at 90% of the item's value.
//!
//! Stat ids and base types come from a snapshot of the trade site's data
//! (`data/trade`, refreshed by `data/trade/update.sh`).

use std::collections::HashMap;
use std::sync::LazyLock;

use super::item::{Item, ItemKind, ModifierSource};
use super::prices::encode;

const TRADE_SITE: &str = "https://www.pathofexile.com";

/// The trade site's stats and base types.
pub struct TradeData {
    /// By text with each number written `#` (see [`numbers`]).
    stats: HashMap<String, Stat>,
    bases: Vec<&'static str>,
}

struct Stat {
    id: &'static str,
    /// For each number in the stat's text: whether it's a value (`#`)
    /// rather than part of the wording (`a 8 second Cooldown`).
    values: Vec<bool>,
}

impl TradeData {
    /// The snapshot built into the program, read on first use.
    pub fn bundled() -> &'static TradeData {
        static DATA: LazyLock<TradeData> =
            LazyLock::new(|| TradeData::read(include_str!("../../data/trade/stats.tsv"), include_str!("../../data/trade/bases.txt")));
        &DATA
    }

    fn read(stats_tsv: &'static str, bases: &'static str) -> TradeData {
        let mut stats: HashMap<String, Stat> = HashMap::new();
        for (id, text) in stats_tsv.lines().filter_map(|line| line.split_once('\t')) {
            let (key, numbers) = numbers(&text.replace("\\n", "\n"));
            let values = numbers.iter().map(|number| number.is_none()).collect();
            // An explicit stat wins over a pseudo one with the same text.
            let explicit = id.starts_with("explicit.");
            match stats.get(&key) {
                Some(known) if !(explicit && !known.id.starts_with("explicit.")) => {}
                _ => {
                    stats.insert(key, Stat { id, values });
                }
            }
        }
        TradeData { stats, bases: bases.lines().filter(|base| !base.is_empty()).collect() }
    }

    /// The longest base type named in `line` (`Frosted Vaal Axe of
    /// Recuperation` → `Vaal Axe`).
    fn base_type(&self, line: &str) -> Option<&'static str> {
        let named = |base: &&str| {
            line.match_indices(*base).any(|(at, _)| {
                let before = line[..at].chars().next_back();
                let after = line[at + base.len()..].chars().next();
                before.is_none_or(|c| c == ' ') && after.is_none_or(|c| c == ' ')
            })
        };
        self.bases.iter().copied().filter(named).max_by_key(|base| base.len())
    }

    /// The stat a modifier's text stands for, local to the item if it can be.
    fn stat(&self, key: &str, locality: Locality) -> Option<&Stat> {
        if locality.takes_local(key)
            && let Some(local) = self.stats.get(&format!("{key} (Local)"))
        {
            return Some(local);
        }
        self.stats.get(key)
    }
}

/// Which "(Local)" stats apply to an item's own modifiers.
#[derive(Clone, Copy)]
enum Locality {
    Weapon,
    Armour,
    None,
}

impl Locality {
    fn of(class: &str) -> Locality {
        const WEAPONS: [&str; 16] = [
            "Claws", "Daggers", "Rune Daggers", "Wands", "One Hand Swords", "Thrusting One Hand Swords", "One Hand Axes", "One Hand Maces",
            "Sceptres", "Bows", "Staves", "Warstaves", "Two Hand Swords", "Two Hand Axes", "Two Hand Maces", "Fishing Rods",
        ];
        const ARMOUR: [&str; 5] = ["Body Armours", "Helmets", "Gloves", "Boots", "Shields"];
        if WEAPONS.contains(&class) {
            Locality::Weapon
        } else if ARMOUR.contains(&class) {
            Locality::Armour
        } else {
            Locality::None
        }
    }

    /// Armour's local stats are its defences; a weapon's are the rest.
    fn takes_local(self, key: &str) -> bool {
        let defence = ["Armour", "Evasion", "Energy Shield"].iter().any(|word| key.contains(word));
        match self {
            Locality::Weapon => !defence,
            Locality::Armour => defence,
            Locality::None => false,
        }
    }
}

/// A trade site search for one item.
#[derive(Debug)]
pub struct TradeQuery<'d> {
    pub base_type: Option<&'d str>,
    pub filters: Vec<StatFilter<'d>>,
    /// Explicit modifiers the trade data has no stat for, as the item shows them.
    pub excluded: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatFilter<'d> {
    pub id: &'d str,
    /// `None`: the stat at any value.
    pub min: Option<f64>,
}

// Pseudo stats: totals over every modifier, as the trade site counts them.
const TOTAL_LIFE: &str = "pseudo.pseudo_total_life";
const TOTAL_ELEMENTAL_RESISTANCE: &str = "pseudo.pseudo_total_elemental_resistance";
const TOTAL_CHAOS_RESISTANCE: &str = "pseudo.pseudo_total_chaos_resistance";
const TOTAL_ATTRIBUTES: [&str; 3] = ["pseudo.pseudo_total_strength", "pseudo.pseudo_total_dexterity", "pseudo.pseudo_total_intelligence"];

const LIFE: &str = "+# to maximum Life";

/// Resistance stats: how many elemental resistances each gives, and whether
/// it gives chaos resistance.
const RESISTANCES: [(&str, f64, bool); 12] = [
    ("+#% to all Resistances", 3.0, true),
    ("+#% to all Elemental Resistances", 3.0, false),
    ("+#% to Fire Resistance", 1.0, false),
    ("+#% to Cold Resistance", 1.0, false),
    ("+#% to Lightning Resistance", 1.0, false),
    ("+#% to Fire and Cold Resistances", 2.0, false),
    ("+#% to Fire and Lightning Resistances", 2.0, false),
    ("+#% to Cold and Lightning Resistances", 2.0, false),
    ("+#% to Chaos Resistance", 0.0, true),
    ("+#% to Fire and Chaos Resistances", 1.0, true),
    ("+#% to Cold and Chaos Resistances", 1.0, true),
    ("+#% to Lightning and Chaos Resistances", 1.0, true),
];

/// Attribute stats: whether each gives strength, dexterity, intelligence.
const ATTRIBUTES: [(&str, [bool; 3]); 7] = [
    ("+# to all Attributes", [true, true, true]),
    ("+# to Strength", [true, false, false]),
    ("+# to Dexterity", [false, true, false]),
    ("+# to Intelligence", [false, false, true]),
    ("+# to Strength and Dexterity", [true, true, false]),
    ("+# to Strength and Intelligence", [true, false, true]),
    ("+# to Dexterity and Intelligence", [false, true, true]),
];

/// A stat's value summed over modifiers, with the most decimals any had.
#[derive(Clone, Copy, Default)]
struct Total {
    value: f64,
    decimals: u8,
}

impl Total {
    fn add(&mut self, value: f64, decimals: u8) {
        self.value += value;
        self.decimals = self.decimals.max(decimals);
    }

    /// 90% of the value, rounded down to its decimals; none for a value
    /// that isn't positive (a `reduced` modifier).
    fn minimum(self) -> Option<f64> {
        let scale = 10f64.powi(i32::from(self.decimals));
        (self.value > 0.0).then(|| (self.value * 9.0 * scale / 10.0).floor() / scale)
    }
}

impl<'d> TradeQuery<'d> {
    /// The search for `item`; `None` for items poe.ninja prices.
    pub fn new(item: &Item, data: &'d TradeData) -> Option<TradeQuery<'d>> {
        let ItemKind::Other { base_line, modifiers, .. } = &item.kind else { return None };
        let locality = Locality::of(&item.class);
        let mut life: Option<Total> = None;
        let (mut elemental, mut chaos) = (Total::default(), Total::default());
        let mut attributes = [Total::default(); 3];
        let mut stats: Vec<(&'d str, Total)> = Vec::new();
        let mut excluded = Vec::new();
        for modifier in modifiers {
            let (key, numbers) = numbers(&modifier.text);
            let first = numbers.first().copied().flatten().unwrap_or_default();
            if key == LIFE {
                life.get_or_insert_default().add(first.0, first.1);
            } else if let Some(&(_, elements, gives_chaos)) = RESISTANCES.iter().find(|(text, ..)| *text == key) {
                if elements > 0.0 {
                    elemental.add(first.0 * elements, first.1);
                }
                if gives_chaos {
                    chaos.add(first.0, first.1);
                }
            } else if let Some((_, gives)) = ATTRIBUTES.iter().find(|(text, _)| *text == key) {
                for (total, _) in attributes.iter_mut().zip(gives).filter(|(_, gives)| **gives) {
                    total.add(first.0, first.1);
                }
            } else if matches!(modifier.source, ModifierSource::Explicit | ModifierSource::Fractured) {
                match stat_value(data, &key, &numbers, locality) {
                    Some((id, value, decimals)) => match stats.iter_mut().find(|(known, _)| *known == id) {
                        Some((_, total)) => total.add(value, decimals),
                        None => stats.push((id, Total { value, decimals })),
                    },
                    None => excluded.push(modifier.text.clone()),
                }
            }
        }
        // The trade site counts half of strength toward life, once there's life.
        if let Some(life) = &mut life {
            life.add(attributes[0].value / 2.0, 0);
        }
        let mut filters = Vec::with_capacity(stats.len() + 6);
        let pseudo = [(TOTAL_LIFE, life.unwrap_or_default()), (TOTAL_ELEMENTAL_RESISTANCE, elemental), (TOTAL_CHAOS_RESISTANCE, chaos)];
        let totals = pseudo.into_iter().chain(TOTAL_ATTRIBUTES.into_iter().zip(attributes));
        filters.extend(totals.filter(|(_, total)| total.value > 0.0).map(|(id, total)| StatFilter { id, min: total.minimum() }));
        filters.extend(stats.into_iter().map(|(id, total)| StatFilter { id, min: total.minimum() }));
        Some(TradeQuery { base_type: data.base_type(base_line), filters, excluded })
    }

    /// The search as the trade site's API and search page take it: online
    /// sellers, any non-unique rarity, cheapest first.
    pub fn to_json(&self) -> String {
        let filters: Vec<serde_json::Value> = self
            .filters
            .iter()
            .map(|filter| match filter.min {
                Some(min) => serde_json::json!({ "id": filter.id, "value": { "min": min } }),
                None => serde_json::json!({ "id": filter.id }),
            })
            .collect();
        let mut query = serde_json::json!({
            "status": { "option": "online" },
            "stats": [{ "type": "and", "filters": filters }],
            "filters": { "type_filters": { "filters": { "rarity": { "option": "nonunique" } } } },
        });
        if let Some(base_type) = self.base_type {
            query["type"] = base_type.into();
        }
        serde_json::json!({ "query": query, "sort": { "price": "asc" } }).to_string()
    }

    /// The trade site's search page with this search filled in.
    pub fn site_url(&self, league: &str) -> String {
        format!("{TRADE_SITE}/trade/search/{}?q={}", encode(league), encode(&self.to_json()))
    }
}

/// A number in a modifier's text and its decimal places.
type Number = (f64, u8);

/// The text with each number written `#`, and the numbers: `None` where the
/// text already had a `#`.
fn numbers(text: &str) -> (String, Vec<Option<Number>>) {
    let mut key = String::with_capacity(text.len());
    let mut numbers = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        if c == '#' {
            key.push('#');
            numbers.push(None);
        } else if c.is_ascii_digit() {
            let mut end = start + 1;
            let mut decimals = None;
            while let Some(&(at, next)) = chars.peek() {
                let is_point = next == '.' && decimals.is_none() && text[at + 1..].starts_with(|c: char| c.is_ascii_digit());
                if !(next.is_ascii_digit() || is_point) {
                    break;
                }
                if is_point {
                    decimals = Some(at);
                }
                end = at + 1;
                chars.next();
            }
            let places = decimals.map_or(0, |point| (end - point - 1) as u8);
            key.push('#');
            numbers.push(text[start..end].parse().ok().map(|value| (value, places)));
        } else {
            key.push(c);
        }
    }
    (key, numbers)
}

/// The trade stat for a modifier, its value (the average of a range like
/// `Adds 3 to 7`) and decimals. `reduced` and `less` are searched as
/// negative `increased` and `more` when the trade site only has those.
fn stat_value<'d>(data: &'d TradeData, key: &str, numbers: &[Option<Number>], locality: Locality) -> Option<(&'d str, f64, u8)> {
    let (stat, sign) = match data.stat(key, locality) {
        Some(stat) => (stat, 1.0),
        None => {
            let flipped = key.replacen(" reduced ", " increased ", 1).replacen(" less ", " more ", 1);
            (data.stat(&flipped, locality).filter(|_| flipped != key)?, -1.0)
        }
    };
    let values: Vec<Number> = stat.values.iter().zip(numbers).filter(|(is_value, _)| **is_value).filter_map(|(_, number)| *number).collect();
    if values.is_empty() {
        return Some((stat.id, 0.0, 0));
    }
    let value = values.iter().map(|(value, _)| value).sum::<f64>() / values.len() as f64;
    let decimals = values.iter().map(|(_, decimals)| *decimals).max().unwrap_or(0);
    Some((stat.id, sign * value, decimals))
}
