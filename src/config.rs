//! User settings in `$XDG_CONFIG_HOME/poe-overlay/config.toml`. A missing
//! file or key means the default.

use std::path::PathBuf;

use serde::Deserialize;

use crate::poe1::route::RouteOptions;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub route: RouteOptions,
}

impl Config {
    /// Reads the config file. An unreadable or invalid file is reported on
    /// stderr and the defaults are used, so a typo never stops the overlay.
    pub fn load() -> Config {
        let Some(path) = path() else { return Config::default() };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Config::default(),
            Err(err) => {
                eprintln!("poe-overlay: cannot read {}: {err}; using defaults", path.display());
                return Config::default();
            }
        };
        toml::from_str(&text).unwrap_or_else(|err| {
            eprintln!("poe-overlay: {}: {err}; using defaults", path.display());
            Config::default()
        })
    }
}

fn path() -> Option<PathBuf> {
    let dir = match std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(dir.join("poe-overlay/config.toml"))
}
