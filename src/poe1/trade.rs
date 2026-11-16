//! Trade site searches for the items poe.ninja doesn't price (rare, magic and
//! normal items): the base type, the life, resistance and attribute totals,
//! and the other explicit modifiers, each at 90% of the item's value.
//!
//! Stat ids and base types come from a snapshot of the trade site's data
//! (`data/trade`, refreshed by `data/trade/update.sh`).

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

use super::item::{Item, ItemKind, ModifierSource};

/// Where the trade site is; its API is under `/api/trade`.
pub(crate) const TRADE_SITE: &str = "https://www.pathofexile.com";

/// The trade site's stats and base types.
pub struct TradeData {
    /// By text with each number written `#` (see [`numbers`]); several where
    /// only numbers in the wording differ (`Onslaught for 4 / 10 seconds`).
    stats: HashMap<String, Vec<Stat>>,
    /// "(Local)" stats, by their text without that.
    local: HashMap<String, Vec<Stat>>,
    bases: Vec<&'static str>,
}

struct Stat {
    id: &'static str,
    /// The numbers in the stat's text: `None` for a value (`#`), else part
    /// of the wording (`a 8 second Cooldown`).
    numbers: Vec<Option<f64>>,
    /// As the trade site words it.
    text: Cow<'static, str>,
}

impl Stat {
    /// Whether a modifier with `numbers` reads like this stat: same wording
    /// numbers.
    fn fits(&self, numbers: &[Option<Value>]) -> bool {
        self.numbers.iter().zip(numbers).all(|(wording, number)| wording.is_none_or(|wording| number.is_some_and(|number| number.value == wording)))
    }
}

impl TradeData {
    /// The snapshot built into the program, read on first use.
    pub fn bundled() -> &'static TradeData {
        static DATA: LazyLock<TradeData> =
            LazyLock::new(|| TradeData::read(include_str!("../../data/trade/stats.tsv"), include_str!("../../data/trade/bases.txt")));
        &DATA
    }

    fn read(stats_tsv: &'static str, bases: &'static str) -> TradeData {
        let (mut stats, mut local) = (HashMap::<String, Vec<Stat>>::new(), HashMap::<String, Vec<Stat>>::new());
        for (id, text) in stats_tsv.lines().filter_map(|line| line.split_once('\t')) {
            let text = if text.contains("\\n") { Cow::Owned(text.replace("\\n", "\n")) } else { Cow::Borrowed(text) };
            let (key, numbers) = numbers(&text);
            let stat = Stat { id, numbers: numbers.iter().map(|number| number.map(|number| number.value)).collect(), text };
            match key.strip_suffix(" (Local)") {
                Some(key) => local.entry(key.to_owned()).or_default().push(stat),
                None => stats.entry(key).or_default().push(stat),
            }
        }
        // An explicit stat wins over a pseudo one with the same text.
        for candidates in stats.values_mut().chain(local.values_mut()) {
            candidates.sort_by_key(|stat| !stat.id.starts_with("explicit."));
        }
        TradeData { stats, local, bases: bases.lines().filter(|base| !base.is_empty()).collect() }
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
    fn stat(&self, key: &str, numbers: &[Option<Value>], locality: Locality) -> Option<&Stat> {
        let local = if locality.takes_local(key) { fitting(self.local.get(key), numbers) } else { None };
        local.or_else(|| fitting(self.stats.get(key), numbers))
    }
}

/// The candidate a modifier with `numbers` reads like: the one matching the
/// most wording numbers (`Pierce 2` over `Pierce #`), explicit first.
fn fitting<'s>(candidates: Option<&'s Vec<Stat>>, numbers: &[Option<Value>]) -> Option<&'s Stat> {
    let wording = |stat: &&Stat| std::cmp::Reverse(stat.numbers.iter().filter(|number| number.is_some()).count());
    candidates?.iter().filter(|stat| stat.fits(numbers)).min_by_key(wording)
}

/// Which "(Local)" stats apply to an item's own modifiers.
#[derive(Clone, Copy)]
enum Locality {
    Weapon,
    Armour,
    Neither,
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
            Locality::Neither
        }
    }

    /// Armour's local stats are its defences; a weapon's are the rest.
    fn takes_local(self, key: &str) -> bool {
        let defence = ["Armour", "Evasion", "Energy Shield"].iter().any(|word| key.contains(word));
        match self {
            Locality::Weapon => !defence,
            Locality::Armour => defence,
            Locality::Neither => false,
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
    /// Implicit, crafted and enchant modifiers that aren't in a total: not
    /// searched by choice.
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatFilter<'d> {
    pub id: &'d str,
    /// The trade site's wording, each value written `#`.
    pub text: &'d str,
    /// `None`: the stat at any value.
    pub min: Option<f64>,
    /// `None`: no upper bound; only the player sets one.
    pub max: Option<f64>,
    /// Whether the search uses it; the player can switch it off.
    pub enabled: bool,
}

/// A typed minimum or maximum that isn't a number.
#[derive(Debug, PartialEq)]
pub struct NotANumber;

/// A minimum or maximum as the player types it: nothing for no bound.
pub fn bound(text: &str) -> Result<Option<f64>, NotANumber> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    text.parse::<f64>().ok().filter(|value| value.is_finite()).map(Some).ok_or(NotANumber)
}

// Pseudo stats (id, wording): totals over every modifier, as the trade site
// counts them.
const TOTAL_LIFE: (&str, &str) = ("pseudo.pseudo_total_life", "+# total maximum Life");
const TOTAL_ELEMENTAL_RESISTANCE: (&str, &str) = ("pseudo.pseudo_total_elemental_resistance", "+#% total Elemental Resistance");
const TOTAL_CHAOS_RESISTANCE: (&str, &str) = ("pseudo.pseudo_total_chaos_resistance", "+#% total to Chaos Resistance");
const TOTAL_ATTRIBUTES: [(&str, &str); 3] = [
    ("pseudo.pseudo_total_strength", "+# total to Strength"),
    ("pseudo.pseudo_total_dexterity", "+# total to Dexterity"),
    ("pseudo.pseudo_total_intelligence", "+# total to Intelligence"),
];

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

/// A number in a modifier's text, or a stat's value summed over modifiers,
/// with its (most) decimal places.
#[derive(Debug, Clone, Copy, Default)]
struct Value {
    value: f64,
    decimals: u8,
}

impl Value {
    fn add(&mut self, other: Value) {
        self.value += other.value;
        self.decimals = self.decimals.max(other.decimals);
    }

    fn times(self, factor: f64) -> Value {
        Value { value: self.value * factor, ..self }
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
        let mut life: Option<Value> = None;
        let (mut elemental, mut chaos) = (Value::default(), Value::default());
        let mut attributes = [Value::default(); 3];
        let mut stats: Vec<(&'d Stat, Value)> = Vec::new();
        let (mut excluded, mut skipped) = (Vec::new(), Vec::new());
        let mut rest = modifiers.iter().peekable();
        while let Some(modifier) = rest.next() {
            let searched = matches!(modifier.source, ModifierSource::Explicit | ModifierSource::Fractured);
            // Some stats span two lines.
            if searched && let Some(next) = rest.peek().filter(|next| next.source == modifier.source) {
                let (key, numbers) = numbers(&format!("{}\n{}", modifier.text, next.text));
                if let Some((stat, value)) = stat_value(data, &key, &numbers, locality) {
                    add_stat(&mut stats, stat, value);
                    rest.next();
                    continue;
                }
            }
            let (key, numbers) = numbers(&modifier.text);
            let first = numbers.first().copied().flatten().unwrap_or_default();
            if key == LIFE {
                life.get_or_insert_default().add(first);
            } else if let Some(&(_, elements, gives_chaos)) = RESISTANCES.iter().find(|(text, ..)| *text == key) {
                if elements > 0.0 {
                    elemental.add(first.times(elements));
                }
                if gives_chaos {
                    chaos.add(first);
                }
            } else if let Some((_, gives)) = ATTRIBUTES.iter().find(|(text, _)| *text == key) {
                for (total, _) in attributes.iter_mut().zip(gives).filter(|(_, gives)| **gives) {
                    total.add(first);
                }
            } else if !searched {
                skipped.push(modifier.text.clone());
            } else {
                match stat_value(data, &key, &numbers, locality) {
                    Some((stat, value)) => add_stat(&mut stats, stat, value),
                    None => excluded.push(modifier.text.clone()),
                }
            }
        }
        // The trade site counts half of strength toward life, once there's life.
        if let Some(life) = &mut life {
            life.add(Value { value: attributes[0].value / 2.0, decimals: 0 });
        }
        let mut filters = Vec::with_capacity(stats.len() + 6);
        let pseudo = [(TOTAL_LIFE, life.unwrap_or_default()), (TOTAL_ELEMENTAL_RESISTANCE, elemental), (TOTAL_CHAOS_RESISTANCE, chaos)];
        let totals = pseudo.into_iter().chain(TOTAL_ATTRIBUTES.into_iter().zip(attributes));
        let filter = |(id, text), total: Value| StatFilter { id, text, min: total.minimum(), max: None, enabled: true };
        filters.extend(totals.filter(|(_, total)| total.value > 0.0).map(|(stat, total)| filter(stat, total)));
        filters.extend(stats.into_iter().map(|(stat, total)| filter((stat.id, &stat.text), total)));
        Some(TradeQuery { base_type: data.base_type(base_line), filters, excluded, skipped })
    }

    /// The search as the trade site's API takes it: online sellers, any
    /// non-unique rarity, cheapest first; switched-off filters left out.
    pub fn to_json(&self) -> String {
        let filters: Vec<serde_json::Value> = self
            .filters
            .iter()
            .filter(|filter| filter.enabled)
            .map(|filter| {
                let mut value = serde_json::Map::new();
                for (side, bound) in [("min", filter.min), ("max", filter.max)] {
                    if let Some(bound) = bound {
                        value.insert(side.into(), bound.into());
                    }
                }
                if value.is_empty() {
                    serde_json::json!({ "id": filter.id })
                } else {
                    serde_json::json!({ "id": filter.id, "value": value })
                }
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
}

/// Adds `value` to the stat's total, the same stat on two modifiers summed.
fn add_stat<'d>(stats: &mut Vec<(&'d Stat, Value)>, stat: &'d Stat, value: Value) {
    match stats.iter_mut().find(|(known, _)| known.id == stat.id) {
        Some((_, total)) => total.add(value),
        None => stats.push((stat, value)),
    }
}

/// The text with each number written `#`, and the numbers: `None` where the
/// text already had a `#`.
fn numbers(text: &str) -> (String, Vec<Option<Value>>) {
    let mut key = String::with_capacity(text.len());
    let mut numbers = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        if c == '#' {
            key.push('#');
            numbers.push(None);
        } else if c.is_ascii_digit() {
            let mut end = start + 1;
            let mut point = None;
            while let Some(&(at, next)) = chars.peek() {
                let is_point = next == '.' && point.is_none() && text[at + 1..].starts_with(|c: char| c.is_ascii_digit());
                if !(next.is_ascii_digit() || is_point) {
                    break;
                }
                if is_point {
                    point = Some(at);
                }
                end = at + 1;
                chars.next();
            }
            let decimals = point.map_or(0, |point| (end - point - 1) as u8);
            key.push('#');
            numbers.push(text[start..end].parse().ok().map(|value| Value { value, decimals }));
        } else {
            key.push(c);
        }
    }
    (key, numbers)
}

/// The trade stat for a modifier's text, and its value: the average of a
/// range like `Adds 3 to 7`, or 0 for a stat without values. `reduced` and
/// `less` are searched as negative `increased` and `more` when the trade
/// site only has those.
fn stat_value<'d>(data: &'d TradeData, key: &str, numbers: &[Option<Value>], locality: Locality) -> Option<(&'d Stat, Value)> {
    let (stat, sign) = match data.stat(key, numbers, locality) {
        Some(stat) => (stat, 1.0),
        None => {
            let flipped = key.replacen(" reduced ", " increased ", 1).replacen(" less ", " more ", 1);
            (data.stat(&flipped, numbers, locality).filter(|_| flipped != key)?, -1.0)
        }
    };
    let values = stat.numbers.iter().zip(numbers).filter(|(wording, _)| wording.is_none()).filter_map(|(_, number)| *number);
    let (sum, count, decimals) = values.fold((0.0, 0u32, 0), |(sum, count, decimals), number| (sum + number.value, count + 1, decimals.max(number.decimals)));
    let value = if count == 0 { 0.0 } else { sign * sum / f64::from(count) };
    Some((stat, Value { value, decimals }))
}
