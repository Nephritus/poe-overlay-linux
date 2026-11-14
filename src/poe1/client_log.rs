//! Events read from the game's Client.txt log.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    /// The Character is entering an Area, identified by the game's area ID.
    AreaEntered { area_id: String },
}

/// Parses one Client.txt line: `<date> <time> <ticks> <hex> [<LEVEL> Client <pid>] <message>`.
pub fn parse_line(line: &str) -> Option<LogEvent> {
    let message = message(line)?;
    // `Generating level 5 area "1_1_4_1" with seed 4172808522`, logged as the
    // Area loads, about a second before `You have entered …`.
    let rest = message.strip_prefix("Generating level ")?;
    let (_level, rest) = rest.split_once(" area \"")?;
    let (area_id, _) = rest.split_once('"')?;
    Some(LogEvent::AreaEntered { area_id: area_id.to_owned() })
}

/// The text after the `[… Client <pid>] ` tag.
fn message(line: &str) -> Option<&str> {
    let tag = line.find(" [")?;
    let (_, after_tag) = line[tag..].split_once("] ")?;
    Some(after_tag)
}
