//! Events read from the game's Client.txt log.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::trade_request::{self, TradeRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    /// The Character is entering an Area, identified by the game's area ID.
    AreaEntered { area_id: String },
    /// A Character reached a new level; the only line naming the Character.
    CharacterLevelled { name: String },
    /// A whisper in the trade site's template, to or from the player.
    TradeRequest(TradeRequest),
}

/// Parses one Client.txt line: `<date> <time> <ticks> <hex> [<LEVEL> Client <pid>] <message>`.
pub fn parse_line(line: &str) -> Option<LogEvent> {
    let message = message(line)?;
    if message.starts_with('@') {
        return trade_request::parse(message).map(LogEvent::TradeRequest);
    }
    area_entered(message).or_else(|| character_levelled(message))
}

/// Whether `path` can be the game's Client.txt; the problem, in words, if
/// not. Only full paths: the overlay doesn't run from the game's folder.
pub fn check_path(path: &Path) -> Result<(), String> {
    let shown = path.display();
    if !path.is_absolute() {
        return Err(format!("{shown} isn't a full path; choose the file or start the path with /."));
    }
    match File::open(path) {
        Ok(file) if file.metadata().is_ok_and(|meta| meta.is_dir()) => {
            Err(format!("{shown} is a folder; choose Client.txt in the game's logs folder."))
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(format!("No file at {shown}.")),
        Err(err) => Err(format!("Can't read {shown}: {err}.")),
    }
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
    level.trim().parse::<u32>().ok()?;
    let (name, _class) = who.strip_suffix(')')?.split_once(" (")?;
    Some(LogEvent::CharacterLevelled { name: name.to_owned() })
}

/// The Character of the latest level-up already in the log, if any: the best
/// guess at who is playing before this session logs a level-up. Reads from
/// the end in chunks, since Client.txt grows to hundreds of megabytes.
pub fn last_character(log: &Path) -> Option<String> {
    const CHUNK: u64 = 64 * 1024;
    let mut file = File::open(log).ok()?;
    let mut end = file.metadata().ok()?.len();
    // Bytes before the first line break seen so far: possibly a partial line.
    let mut carry: Vec<u8> = Vec::new();
    let mut buf = Vec::new();
    while end > 0 {
        let start = end.saturating_sub(CHUNK);
        buf.resize((end - start) as usize, 0);
        file.seek(SeekFrom::Start(start)).ok()?;
        file.read_exact(&mut buf).ok()?;
        buf.extend_from_slice(&carry);
        let mut lines = buf.rsplit(|b| matches!(b, b'\r' | b'\n')).peekable();
        while let Some(line) = lines.next() {
            if lines.peek().is_none() && start > 0 {
                carry.clear();
                carry.extend_from_slice(line);
                break;
            }
            if let Some(LogEvent::CharacterLevelled { name }) = std::str::from_utf8(line).ok().and_then(parse_line) {
                return Some(name);
            }
        }
        end = start;
    }
    None
}

/// The text after the `[… Client <pid>] ` tag.
fn message(line: &str) -> Option<&str> {
    let tag = line.find(" [")?;
    let (_, after_tag) = line[tag..].split_once("] ")?;
    Some(after_tag)
}
