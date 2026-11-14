//! Writing files the player or the overlay depends on.

use std::fs;
use std::io;
use std::path::Path;

/// Replaces `path`'s contents in one step (temporary file, then rename), so a
/// crash leaves the old or the new text, never half. Creates the folder if
/// needed. A symlinked file (dotfile managers) keeps its link: the target is
/// replaced, keeping its permissions.
pub fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let target = match fs::canonicalize(path) {
        Ok(target) => target,
        Err(err) if err.kind() == io::ErrorKind::NotFound => path.to_owned(),
        Err(err) => return Err(err),
    };
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = target.clone().into_os_string();
    tmp.push(".poe-overlay-tmp");
    fs::write(&tmp, text)?;
    if let Ok(meta) = fs::metadata(&target) {
        fs::set_permissions(&tmp, meta.permissions())?;
    }
    fs::rename(&tmp, &target)
}
