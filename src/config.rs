//! User settings in `$XDG_CONFIG_HOME/poe-overlay/config.toml`. A missing
//! file or key means the default.

use std::fmt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::hotkeys::Hotkeys;
use crate::poe1::route::RouteOptions;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub route: RouteOptions,
    pub prices: PricesConfig,
    pub hotkeys: Hotkeys,
    pub game: GameConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PricesConfig {
    /// League to price in; unset means the league poe.ninja lists first
    /// (the current challenge league).
    pub league: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GameConfig {
    /// The game's Client.txt, for installs Steam doesn't know (Lutris,
    /// Heroic); unset means found through Steam.
    pub client_txt: Option<PathBuf>,
}

impl Config {
    /// Reads the config file; a missing file is the defaults. An unreadable
    /// or invalid file is an error message naming the file, so callers can
    /// fall back to the defaults and tell the player.
    pub fn load() -> Result<Config, String> {
        let Some(path) = path() else { return Ok(Config::default()) };
        let text = read_text(&path)?;
        Config::parse(&text).map_err(|err| format!("{}: {err}", path.display()))
    }

    /// Reads the config file's text the way the overlay does.
    pub fn parse(text: &str) -> Result<Config, String> {
        toml::from_str(text).map_err(|err| err.to_string())
    }
}

/// Where the config file is; `None` without `HOME`.
pub fn path() -> Option<PathBuf> {
    Some(crate::xdg::config_home()?.join("poe-overlay/config.toml"))
}

/// The config file's text; empty if there's no file yet.
pub fn read_text(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(format!("cannot read {}: {err}", path.display())),
    }
}

/// Checks the config file once a second on a thread of its own; the
/// receiver yields the config, loaded anew, after each change since this
/// call (call it before loading, so no change is missed).
pub fn watch() -> async_channel::Receiver<Result<Config, String>> {
    let (tx, rx) = async_channel::unbounded();
    let Some(path) = path() else { return rx };
    let stamp = |path: &Path| std::fs::metadata(path).ok().map(|meta| (meta.modified().ok(), meta.len(), meta.ino()));
    let mut last = stamp(&path);
    std::thread::spawn(move || {
        while !tx.is_closed() {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let now = stamp(&path);
            if now != last {
                last = now;
                if tx.send_blocking(Config::load()).is_err() {
                    return;
                }
            }
        }
    });
    rx
}

/// The config file's text, for changing the settings the Settings window
/// changes while keeping everything else as the player wrote it.
#[derive(Debug)]
pub struct ConfigFile {
    doc: toml_edit::DocumentMut,
}

impl ConfigFile {
    /// A file with a syntax error is refused: rewriting it would lose what
    /// the player meant.
    pub fn parse(text: &str) -> Result<ConfigFile, String> {
        text.parse().map(|doc| ConfigFile { doc }).map_err(|err: toml_edit::TomlError| err.to_string())
    }

    pub fn league(&self) -> Option<&str> {
        self.get("prices", "league")
    }

    pub fn client_txt(&self) -> Option<&Path> {
        self.get("game", "client_txt").map(Path::new)
    }

    /// `None` goes back to the league poe.ninja lists first.
    pub fn set_league(&mut self, league: Option<&str>) {
        self.set("prices", "league", league);
    }

    /// `None` goes back to finding Client.txt through Steam.
    pub fn set_client_txt(&mut self, path: Option<&Path>) {
        self.set("game", "client_txt", path.map(|path| path.to_string_lossy()).as_deref());
    }

    fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.doc.get(section)?.get(key)?.as_str()
    }

    /// Sets `section.key`, or removes it (and the section, once empty).
    fn set(&mut self, section: &str, key: &str, value: Option<&str>) {
        match value {
            Some(value) => {
                let item = self.doc.entry(section).or_insert_with(toml_edit::table);
                if item.as_table_like().is_none() {
                    *item = toml_edit::table();
                }
                item[key] = toml_edit::value(value);
            }
            None => {
                let Some(table) = self.doc.get_mut(section).and_then(toml_edit::Item::as_table_like_mut) else { return };
                table.remove(key);
                if table.is_empty() {
                    self.doc.remove(section);
                }
            }
        }
    }
}

impl fmt::Display for ConfigFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.doc.fmt(f)
    }
}
