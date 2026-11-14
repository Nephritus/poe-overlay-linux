//! Item text as the game copies it to the clipboard (Ctrl+Alt+C): a header
//! with the item class, rarity and name, then sections separated by
//! `--------` lines. English client only.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// e.g. `Body Armours`, `Stackable Currency`.
    pub class: String,
    /// The unique's or gem's name, the currency or card name; an
    /// unidentified unique's base type.
    pub name: String,
    pub kind: ItemKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    /// Stackable items traded on the currency exchange: currency, oils,
    /// essences, fragments, scarabs.
    Currency { stack: u32 },
    DivinationCard { stack: u32 },
    Gem {
        level: u32,
        quality: u32,
        corrupted: bool,
        /// The support an imbued gem comes with (`Supported by Level 1
        /// Knockback`); it trades with it.
        imbued: Option<String>,
    },
    /// Identified: its name and base both show.
    Unique { links: u32 },
    /// Its name isn't shown until identified, only `Item::name`, the base.
    UnidentifiedUnique,
    /// Anything else (rare, magic and normal equipment, maps…).
    Other { rarity: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemTextError {
    /// Item text from a client in another language.
    NotEnglish,
    NotAnItem,
}

impl fmt::Display for ItemTextError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            ItemTextError::NotEnglish => "This item text isn't English: Price Check works with the English game client only.",
            ItemTextError::NotAnItem => "The game copied no item: hover over an item and press the hotkey again.",
        })
    }
}

const SEPARATOR: &str = "--------";

pub fn parse(text: &str) -> Result<Item, ItemTextError> {
    // `lines` also drops the `\r` of the game's Windows line endings.
    let mut sections = text.split(SEPARATOR).map(|section| section.lines().filter(|line| !line.is_empty()));
    let mut header = sections.next().ok_or(ItemTextError::NotAnItem)?;
    let Some(class) = header.next().and_then(|line| line.strip_prefix("Item Class: ")) else {
        // Other languages keep the shape: "<label>: <value>" lines and separators.
        let looks_like_item = text.lines().any(|line| line.trim() == SEPARATOR) && text.lines().next().is_some_and(|line| line.contains(": "));
        return Err(if looks_like_item { ItemTextError::NotEnglish } else { ItemTextError::NotAnItem });
    };
    let rarity = header.next().and_then(|line| line.strip_prefix("Rarity: ")).ok_or(ItemTextError::NotAnItem)?;
    let name = header.next().ok_or(ItemTextError::NotAnItem)?;
    let identified = header.next().is_some();
    let sections: Vec<Vec<&str>> = sections.map(Iterator::collect).collect();
    let lines = || sections.iter().flatten().copied();
    let value = |label: &str| lines().find_map(|line| line.strip_prefix(label));
    let stack = value("Stack Size: ").and_then(|size| number(size.split('/').next()?));

    let mut name = name.to_owned();
    let kind = match (rarity, stack) {
        ("Currency", Some(stack)) => ItemKind::Currency { stack },
        ("Divination Card", Some(stack)) => ItemKind::DivinationCard { stack },
        ("Gem", _) => {
            // A Vaal gem shows its plain skill first and the Vaal skill in a
            // section of its own; it trades under the Vaal name.
            let vaal = format!("Vaal {name}");
            if sections.iter().any(|section| section[..] == [vaal.as_str()]) {
                name = vaal;
            }
            ItemKind::Gem {
                level: value("Level: ").and_then(number).unwrap_or(1),
                quality: value("Quality: ").and_then(number).unwrap_or(0),
                corrupted: sections.iter().any(|section| section[..] == ["Corrupted"]),
                imbued: sections.iter().any(|section| section[..] == ["Imbued"]).then(|| imbued_support(lines())).flatten(),
            }
        }
        ("Unique", _) if identified => ItemKind::Unique { links: value("Sockets: ").map_or(0, links) },
        ("Unique", _) => ItemKind::UnidentifiedUnique,
        // Fragments and scarabs are normal items that stack.
        ("Normal", Some(stack)) => ItemKind::Currency { stack },
        (rarity, _) => ItemKind::Other { rarity: rarity.to_owned() },
    };
    Ok(Item { class: class.to_owned(), name, kind })
}

/// `Knockback` from `Supported by Level 1 Knockback`.
fn imbued_support<'a>(mut lines: impl Iterator<Item = &'a str>) -> Option<String> {
    let level_and_name = lines.find_map(|line| line.strip_prefix("Supported by Level "))?;
    Some(level_and_name.split_once(' ')?.1.to_owned())
}

/// The leading whole number of `+20% (augmented)`, `20 (Max)`, `1,234`.
fn number(text: &str) -> Option<u32> {
    let digits: String = text.trim_start_matches('+').chars().take_while(|c| c.is_ascii_digit() || *c == ',').filter(|c| *c != ',').collect();
    digits.parse().ok()
}

/// The largest link group of `B-B-R B-G`: sockets joined by `-` are linked.
fn links(sockets: &str) -> u32 {
    sockets.split_whitespace().map(|group| group.split('-').count() as u32).max().unwrap_or(0)
}
