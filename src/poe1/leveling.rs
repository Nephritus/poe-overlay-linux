//! A leveling session: Route progress that follows Client.txt, belongs to a
//! Character, and is saved so it survives restarts.
//!
//! Client.txt names the Character only when it levels up, and never says when
//! another Character logs in. So the Character is assumed from the log's
//! history at startup, confirmed by its level-ups, and a level-up of another
//! name switches to that Character. On a switch the previous Character goes
//! back to its position at its last confirmation (startup or level-up): Areas
//! entered since then may have been the new Character's.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::client_log::LogEvent;
use super::route::{Position, Route, RouteProgress};

/// Saved Positions, one per Character name, in a TOML file.
#[derive(Debug)]
pub struct ProgressStore {
    path: PathBuf,
    characters: BTreeMap<String, Position>,
}

#[derive(Default, Deserialize)]
struct StoreFile {
    #[serde(default)]
    characters: BTreeMap<String, Position>,
}

#[derive(Serialize)]
struct StoreFileRef<'a> {
    characters: &'a BTreeMap<String, Position>,
}

impl ProgressStore {
    /// Default location: `$XDG_STATE_HOME/poe-overlay/progress.toml`, else
    /// `~/.local/state/…`; the temp directory only if `HOME` is unset.
    pub fn default_path() -> PathBuf {
        let dir = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|d| d.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
            .unwrap_or_else(std::env::temp_dir);
        dir.join("poe-overlay/progress.toml")
    }

    /// Loads `path`; a missing file is an empty store. An unreadable file is
    /// moved aside to `<name>.corrupt` (never overwritten) and the store
    /// starts empty, with a warning for the player.
    pub fn open(path: PathBuf) -> (ProgressStore, Option<String>) {
        let parsed = match fs::read_to_string(&path) {
            Ok(text) => toml::from_str::<StoreFile>(&text).map_err(|err| err.to_string()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(StoreFile::default()),
            Err(err) => Err(err.to_string()),
        };
        let (file, warning) = match parsed {
            Ok(file) => (file, None),
            Err(err) => {
                let aside = path.with_extension("toml.corrupt");
                let kept = match fs::rename(&path, &aside) {
                    Ok(()) => format!("kept as {}", aside.display()),
                    Err(rename) => format!("could not move it aside: {rename}"),
                };
                (StoreFile::default(), Some(format!("saved progress {} unreadable ({err}); {kept}", path.display())))
            }
        };
        (ProgressStore { path, characters: file.characters }, warning)
    }

    fn get(&self, character: &str) -> Option<&Position> {
        self.characters.get(character)
    }

    fn set(&mut self, character: &str, position: Option<Position>) {
        match position {
            Some(position) if self.characters.get(character) == Some(&position) => return,
            Some(position) => {
                self.characters.insert(character.to_owned(), position);
            }
            None if self.characters.remove(character).is_none() => return,
            None => {}
        }
        if let Err(err) = self.save() {
            eprintln!("poe-overlay: cannot save progress to {}: {err}", self.path.display());
        }
    }

    /// Writes the whole file atomically (temporary file, then rename).
    fn save(&self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = toml::to_string(&StoreFileRef { characters: &self.characters }).map_err(std::io::Error::other)?;
        let tmp = self.path.with_extension("toml.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.path)
    }
}

pub struct Leveling {
    progress: RouteProgress,
    store: ProgressStore,
    character: Option<String>,
    /// The Character's saved Position when it was last confirmed; restored on
    /// a switch away from it. `None`: it had no saved progress then.
    confirmed: Option<Position>,
    /// Latest Area entered, to place a Character identified mid-Area.
    last_area: Option<String>,
}

impl Leveling {
    /// Starts with `assumed` (the latest Character in Client.txt history) and
    /// its saved progress, if any.
    pub fn new(route: Route, store: ProgressStore, assumed: Option<String>) -> Self {
        let mut progress = RouteProgress::new(route);
        let confirmed = assumed.as_deref().and_then(|name| store.get(name)).cloned();
        if let Some(position) = &confirmed {
            progress.restore(position);
        }
        Self { progress, store, character: assumed, confirmed, last_area: None }
    }

    pub fn progress(&self) -> &RouteProgress {
        &self.progress
    }

    pub fn character(&self) -> Option<&str> {
        self.character.as_deref()
    }

    /// Applies a Client.txt event; returns whether what's shown changed.
    pub fn handle(&mut self, event: &LogEvent) -> bool {
        match event {
            LogEvent::AreaEntered { area_id } => {
                match &mut self.last_area {
                    Some(last) => last.clone_from(area_id),
                    None => self.last_area = Some(area_id.clone()),
                }
                let changed = self.progress.enter_area(area_id);
                self.save();
                changed
            }
            LogEvent::CharacterLevelled { name, .. } => self.identify(name),
        }
    }

    pub fn next_leg(&mut self) -> bool {
        let moved = self.progress.next_leg();
        self.save();
        moved
    }

    pub fn previous_leg(&mut self) -> bool {
        let moved = self.progress.previous_leg();
        self.save();
        moved
    }

    fn identify(&mut self, name: &str) -> bool {
        if self.character.as_deref() == Some(name) {
            self.confirmed = Some(self.progress.position());
            return false;
        }
        if let Some(previous) = self.character.take() {
            self.store.set(&previous, self.confirmed.take());
        }
        self.character = Some(name.to_owned());
        let changed = match self.store.get(name).cloned() {
            // A returning Character: its saved visit of the Area it's in.
            Some(saved) => {
                self.progress.restore(&saved);
                if let Some(area) = self.last_area.clone() {
                    self.progress.enter_area(&area);
                }
                true
            }
            // A new Character takes the progress made so far this session.
            None => false,
        };
        self.save();
        self.confirmed = Some(self.progress.position());
        changed
    }

    fn save(&mut self) {
        if let Some(name) = &self.character {
            self.store.set(name, Some(self.progress.position()));
        }
    }
}
