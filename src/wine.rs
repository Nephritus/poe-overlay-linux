//! Wine prefix adjustments made before the game starts.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The game's Wine prefix: `WINEPREFIX` (Lutris, Heroic, plain Wine) or
/// Steam's `STEAM_COMPAT_DATA_PATH/pfx` (Proton).
pub fn prefix_from_env() -> Option<PathBuf> {
    if let Some(prefix) = std::env::var_os("WINEPREFIX") {
        return Some(prefix.into());
    }
    std::env::var_os("STEAM_COMPAT_DATA_PATH").map(|compat| Path::new(&compat).join("pfx"))
}

/// Stops Wine locking the cursor to `exes`' windows (`GrabPointer=N`).
///
/// Wine locks the cursor to any full-screen foreground window on a
/// single-monitor desktop and keeps it locked until focus moves. Under
/// Xwayland that becomes a pointer confinement, and the compositor then keeps
/// the cursor out of overlay panels drawn on top of the game.
///
/// A `GrabPointer` value already present for an exe is the user's choice and
/// is left alone. Must run while Wine is not using the prefix: the wineserver
/// rewrites `user.reg` from memory when it exits. Does nothing if the prefix
/// has no `user.reg` yet (Proton creates it on first launch).
pub fn keep_cursor_unlocked(prefix: &Path, exes: &[&str]) -> io::Result<()> {
    let path = prefix.join("user.reg");
    let original = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    let mut reg = original.clone();
    for exe in exes {
        set_grab_pointer_off(&mut reg, exe);
    }
    if reg == original {
        return Ok(());
    }
    let backup = prefix.join("user.reg.poe-overlay-backup");
    if !backup.exists() {
        fs::copy(&path, &backup)?;
    }
    let tmp = prefix.join("user.reg.poe-overlay-tmp");
    fs::write(&tmp, reg)?;
    fs::rename(&tmp, &path)
}

const VALUE: &str = "\"GrabPointer\"=\"N\"\n";

fn set_grab_pointer_off(reg: &mut String, exe: &str) {
    // Registry files escape backslashes in key names.
    let header = format!("[Software\\\\Wine\\\\AppDefaults\\\\{exe}\\\\X11 Driver]");
    let lines: Vec<(usize, &str)> = line_offsets(reg).collect();
    let Some(start) = lines.iter().position(|(_, line)| starts_with_ignore_case(line, &header)) else {
        if !reg.ends_with('\n') {
            reg.push('\n');
        }
        reg.push('\n');
        reg.push_str(&header);
        reg.push('\n');
        reg.push_str(VALUE);
        return;
    };
    let section = lines[start + 1..]
        .iter()
        .take_while(|(_, line)| !line.is_empty() && !line.starts_with('['));
    let mut insert_at = lines.get(start + 1).map_or(reg.len(), |(offset, _)| *offset);
    for (offset, line) in section {
        if starts_with_ignore_case(line, "\"GrabPointer\"=") {
            return;
        }
        if line.starts_with("#time=") {
            insert_at = offset + line.len() + 1;
        }
    }
    reg.insert_str(insert_at.min(reg.len()), VALUE);
}

/// Each line without its newline, with its byte offset.
fn line_offsets(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split_inclusive('\n').scan(0, |offset, line| {
        let start = *offset;
        *offset += line.len();
        Some((start, line.trim_end_matches('\n')))
    })
}

fn starts_with_ignore_case(line: &str, prefix: &str) -> bool {
    line.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}
