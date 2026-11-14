//! Events read from the game's Client.txt log.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    /// The Character is entering an Area, identified by the game's area ID.
    AreaEntered { area_id: String },
    /// A Character reached a new level; the only line naming the Character.
    CharacterLevelled { name: String, class: String, level: u32 },
}

/// Parses one Client.txt line: `<date> <time> <ticks> <hex> [<LEVEL> Client <pid>] <message>`.
pub fn parse_line(line: &str) -> Option<LogEvent> {
    let message = message(line)?;
    area_entered(message).or_else(|| character_levelled(message))
}

/// `Generating level 5 area "1_1_4_1" with seed 4172808522`, logged as the
/// Area loads, about a second before `You have entered …`.
fn area_entered(message: &str) -> Option<LogEvent> {
    let rest = message.strip_prefix("Generating level ")?;
    let (_level, rest) = rest.split_once(" area \"")?;
    let (area_id, _) = rest.split_once('"')?;
    Some(LogEvent::AreaEntered { area_id: area_id.to_owned() })
}

/// `: ExampleExile (Shadow) is now level 3`. Game messages start with `: `;
/// chat lines start with a channel prefix (`#`, `@From`, …) instead.
fn character_levelled(message: &str) -> Option<LogEvent> {
    let rest = message.strip_prefix(": ")?;
    let (who, level) = rest.split_once(" is now level ")?;
    let (name, class) = who.strip_suffix(')')?.split_once(" (")?;
    Some(LogEvent::CharacterLevelled { name: name.to_owned(), class: class.to_owned(), level: level.trim().parse().ok()? })
}

/// The Character of the latest level-up already in the log, if any: the best
/// guess at who is playing before this session logs a level-up.
pub fn last_character(log: &Path) -> Option<String> {
    let bytes = std::fs::read(log).ok()?;
    String::from_utf8_lossy(&bytes).rsplit(['\r', '\n']).find_map(|line| match parse_line(line)? {
        LogEvent::CharacterLevelled { name, .. } => Some(name),
        LogEvent::AreaEntered { .. } => None,
    })
}

/// The text after the `[… Client <pid>] ` tag.
fn message(line: &str) -> Option<&str> {
    let tag = line.find(" [")?;
    let (_, after_tag) = line[tag..].split_once("] ")?;
    Some(after_tag)
}
