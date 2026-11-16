//! XDG base directories. The spec says to ignore relative values.

use std::path::PathBuf;

/// `$XDG_CONFIG_HOME`, else `~/.config`; `None` without `HOME`.
pub fn config_home() -> Option<PathBuf> {
    base("XDG_CONFIG_HOME", ".config")
}

/// `$XDG_STATE_HOME`, else `~/.local/state`; `None` without `HOME`.
pub fn state_home() -> Option<PathBuf> {
    base("XDG_STATE_HOME", ".local/state")
}

/// `$XDG_RUNTIME_DIR` (private to the user, in memory), else the temporary
/// folder.
pub fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|dir| dir.is_absolute()).unwrap_or_else(std::env::temp_dir)
}

fn base(var: &str, under_home: &str) -> Option<PathBuf> {
    match std::env::var_os(var).map(PathBuf::from).filter(|dir| dir.is_absolute()) {
        Some(dir) => Some(dir),
        None => Some(PathBuf::from(std::env::var_os("HOME")?).join(under_home)),
    }
}
