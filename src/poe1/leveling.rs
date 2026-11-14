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
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::client_log::LogEvent;
use super::route::{Position, Route, RouteProgress};

/// Saved Positions, one per Character name, in a TOML file.
#[derive(Debug)]
pub struct ProgressStore {
    path: PathBuf,
    characters: BTreeMap<String, Position>,
    /// False when the file exists but couldn't be read or moved aside:
    /// saving would destroy it.
    writable: bool,
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

    /// Loads `path`; a missing file is an empty store. A file that isn't
    /// valid is moved aside to `<name>.corrupt-<unix time>` and the store
    /// starts empty. If it can't be read or moved aside, nothing is saved
    /// this session so it is never overwritten. Both come with a warning.
    pub fn open(path: PathBuf) -> (ProgressStore, Option<String>) {
        let empty = |path, writable| ProgressStore { path, characters: BTreeMap::new(), writable };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return (empty(path, true), None),
            Err(err) => {
                let warning = format!("cannot read saved progress {}: {err}; progress won't be saved", path.display());
                return (empty(path, false), Some(warning));
            }
        };
        let err = match toml::from_str::<StoreFile>(&text) {
            Ok(file) => return (ProgressStore { path, characters: file.characters, writable: true }, None),
            Err(err) => err,
        };
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let aside = path.with_extension(format!("toml.corrupt-{stamp}"));
        let (writable, outcome) = match fs::rename(&path, &aside) {
            Ok(()) => (true, format!("kept as {}", aside.display())),
            Err(rename) => (false, format!("could not move it aside ({rename}); progress won't be saved")),
        };
        let warning = format!("saved progress {} is invalid ({err}); {outcome}", path.display());
        (empty(path, writable), Some(warning))
    }

    fn get(&self, character: &str) -> Option<&Position> {
        self.characters.get(character)
    }

    /// Records where `progress` is for `character`; writes only on change.
    fn record(&mut self, character: &str, progress: &RouteProgress) {
        match self.characters.get_mut(character) {
            Some(saved) if progress.is_at(saved) => return,
            Some(saved) => progress.save_into(saved),
            None => {
                self.characters.insert(character.to_owned(), progress.position());
            }
        }
        self.write();
    }

    /// Puts `character` back to `position`, or forgets it if `None`.
    fn reset(&mut self, character: &str, position: Option<Position>) {
        let changed = match position {
            Some(position) => self.characters.insert(character.to_owned(), position.clone()) != Some(position),
            None => self.characters.remove(character).is_some(),
        };
        if changed {
            self.write();
        }
    }

    fn write(&self) {
        if self.writable
            && let Err(err) = self.save()
        {
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
        let saved = assumed.as_deref().and_then(|name| store.get(name));
        let confirmed = saved.filter(|position| progress.restore(position)).cloned();
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
            self.confirm();
            return false;
        }
        if let Some(previous) = self.character.take() {
            self.store.reset(&previous, self.confirmed.take());
        }
        // A returning Character resumes its saved visit of the Area it's in;
        // a new one (or one whose saved Area left the Route) takes the
        // progress made so far this session.
        let resumed = self.store.get(name).is_some_and(|saved| self.progress.restore(saved));
        if resumed && let Some(area) = &self.last_area {
            self.progress.enter_area(area);
        }
        self.character = Some(name.to_owned());
        self.save();
        self.confirm();
        resumed
    }

    fn confirm(&mut self) {
        match &mut self.confirmed {
            Some(position) => self.progress.save_into(position),
            None => self.confirmed = Some(self.progress.position()),
        }
    }

    fn save(&mut self) {
        if let Some(name) = &self.character {
            self.store.record(name, &self.progress);
        }
    }
}
