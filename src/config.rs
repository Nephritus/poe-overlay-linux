//! User settings in `$XDG_CONFIG_HOME/poe-overlay/config.toml`. A missing
//! file or key means the default.

use std::path::PathBuf;

use serde::Deserialize;

use crate::hotkeys::Hotkeys;
use crate::poe1::route::RouteOptions;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub route: RouteOptions,
    pub prices: PricesConfig,
    pub hotkeys: Hotkeys,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PricesConfig {
    /// League to price in; unset means the league poe.ninja lists first
    /// (the current challenge league).
    pub league: Option<String>,
}

impl Config {
    /// Reads the config file; a missing file is the defaults. An unreadable
    /// or invalid file is an error message naming the file, so callers can
    /// fall back to the defaults and tell the player.
    pub fn load() -> Result<Config, String> {
        let Some(path) = path() else { return Ok(Config::default()) };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(err) => return Err(format!("cannot read {}: {err}", path.display())),
        };
        toml::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
    }
}

/// Where the config file is; `None` without `HOME`.
pub fn path() -> Option<PathBuf> {
    Some(crate::xdg::config_home()?.join("poe-overlay/config.toml"))
}
