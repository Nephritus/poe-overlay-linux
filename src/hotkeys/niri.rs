//! Hotkeys in niri: binds in `poe-overlay.kdl` next to the player's
//! `config.kdl`, which includes it once. niri reloads both on change.
//!
//! When two files bind the same key, niri lets the later one win without a
//! word, and our include comes last. So the player's config is read the way
//! niri reads it (same KDL parser, includes followed) to find keys that are
//! already taken.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use knuffel::ast::{Literal, Node, Value};
use knuffel::span::{Span, Spanned};

use super::{Fate, Hotkey, Hotkeys, Location, Plan, Planned};
use crate::files::write_atomic;
use crate::running::Action;

const FILE_NAME: &str = "poe-overlay.kdl";

/// Appended to config.kdl and removed again verbatim. `optional=true`: if our
/// file goes missing, niri still loads the rest.
const INCLUDE: &str = "\n// Hotkeys from poe-overlay's config.toml; `poe-overlay setup hotkeys --remove` takes this out.\ninclude optional=true \"./poe-overlay.kdl\"\n";

/// niri stops at this include depth too.
const MAX_DEPTH: usize = 10;

pub struct Niri {
    /// The player's config.kdl.
    config: PathBuf,
}

impl Niri {
    pub fn at(config: impl Into<PathBuf>) -> Niri {
        Niri { config: config.into() }
    }

    /// The niri session we run in, if any, with the config file niri reads.
    pub fn from_env() -> Option<Niri> {
        std::env::var_os("NIRI_SOCKET")?;
        if let Some(path) = std::env::var_os("NIRI_CONFIG").filter(|path| !path.is_empty()) {
            return Some(Niri::at(path));
        }
        Some(Niri::at(crate::xdg::config_home()?.join("niri/config.kdl")))
    }

    fn ours(&self) -> PathBuf {
        self.config.with_file_name(FILE_NAME)
    }

    fn folder(&self) -> &Path {
        self.config.parent().unwrap_or(Path::new(""))
    }

    /// Reads the player's config and decides each hotkey's fate; `program`
    /// is what the binds run.
    pub fn plan(&self, hotkeys: &Hotkeys, program: &Path) -> Result<Plan, String> {
        let mut found = Found { ours: same_file_key(&self.ours()), installed: false, binds: Vec::new(), files: Vec::new() };
        found.scan(&self.config, self.folder(), 0)?;

        let binds: Vec<Planned> = hotkeys
            .iter()
            .map(|(action, key)| {
                let by_hand = found.binds.iter().find(|bind| bind.triggers == Some(action));
                let taken = || found.binds.iter().find(|bind| bind.key.as_ref() == Some(key));
                let fate = match (by_hand, taken()) {
                    (Some(bind), _) => Fate::ByHand(found.location(bind, self.folder())),
                    (None, Some(bind)) => Fate::Taken(found.location(bind, self.folder())),
                    (None, None) => Fate::Bind,
                };
                Planned { action, key: key.clone(), fate }
            })
            .collect();
        let file = binds_file(&binds, program);
        let up_to_date = found.installed && fs::read_to_string(self.ours()).is_ok_and(|text| text == file);
        Ok(Plan { binds, installed: found.installed, up_to_date, file })
    }

    /// Writes our file and, the first time, includes it from config.kdl.
    pub fn apply(&self, plan: &Plan) -> io::Result<()> {
        write_atomic(&self.ours(), &plan.file)?;
        if !plan.installed {
            let mut text = fs::read_to_string(&self.config)?;
            text.push_str(INCLUDE);
            write_atomic(&self.config, &text)?;
        }
        Ok(())
    }

    /// Deletes our file and the include `apply` added. Returns whether there
    /// was anything to remove. An include of our file that `apply` didn't
    /// write (edited by the player) is left, with our file, for the player.
    pub fn remove(&self) -> Result<bool, String> {
        let text = fs::read_to_string(&self.config).map_err(|err| format!("cannot read {}: {err}", self.config.display()))?;
        let kept = text.find(INCLUDE).map(|at| {
            let mut kept = String::with_capacity(text.len() - INCLUDE.len());
            kept.push_str(&text[..at]);
            kept.push_str(&text[at + INCLUDE.len()..]);
            kept
        });
        if kept.is_none() && text.contains(FILE_NAME) {
            return Err(format!(
                "{} includes {FILE_NAME} in a line poe-overlay didn't write; remove that include, then {FILE_NAME}, yourself",
                self.config.display()
            ));
        }
        let deleted = match fs::remove_file(self.ours()) {
            Ok(()) => true,
            Err(err) if err.kind() == io::ErrorKind::NotFound => false,
            Err(err) => return Err(format!("cannot delete {}: {err}", self.ours().display())),
        };
        let Some(kept) = kept else { return Ok(deleted) };
        write_atomic(&self.config, &kept).map_err(|err| format!("cannot write {}: {err}", self.config.display()))?;
        Ok(true)
    }
}

/// What the player's config files hold, as far as hotkeys go.
struct Found {
    ours: PathBuf,
    installed: bool,
    binds: Vec<Bind>,
    /// Every file read, in order; binds point into it.
    files: Vec<PathBuf>,
}

struct Bind {
    /// `None` for triggers we don't bind ourselves (mouse buttons, wheel).
    key: Option<Hotkey>,
    /// The Action it runs, if it runs `poe-overlay trigger <action>`.
    triggers: Option<Action>,
    /// Index into `Found::files`.
    file: usize,
    line: usize,
}

impl Found {
    fn location(&self, bind: &Bind, folder: &Path) -> Location {
        let file = &self.files[bind.file];
        Location { file: file.strip_prefix(folder).unwrap_or(file).to_owned(), line: bind.line }
    }

    /// Reads `path` and the files it includes, collecting binds; `base` is
    /// where its relative includes start.
    fn scan(&mut self, path: &Path, base: &Path, depth: usize) -> Result<(), String> {
        if depth == MAX_DEPTH || self.files.iter().any(|file| file == path) {
            return Ok(());
        }
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            // A missing include is niri's to report.
            Err(err) if err.kind() == io::ErrorKind::NotFound && depth > 0 => return Ok(()),
            Err(err) => return Err(format!("cannot read niri config {}: {err}", path.display())),
        };
        let name = path.to_string_lossy();
        let document = knuffel::parse_ast::<Span>(&name, &text)
            .map_err(|_| format!("niri config {} has a syntax error; `niri validate` shows where", path.display()))?;
        let file = self.files.len();
        self.files.push(path.to_owned());
        // Lines counted so far, up to byte `counted`; binds come in file order.
        let (mut counted, mut line) = (0, 1);
        for node in &document.nodes {
            match &**node.node_name {
                "include" => {
                    let Some(include) = node.arguments.first().and_then(string) else { continue };
                    let included = resolve(include, base);
                    if same_file_key(&included) == self.ours {
                        // Wherever the player moved our include to.
                        self.installed = true;
                        continue;
                    }
                    let base = included.parent().unwrap_or(Path::new("")).to_owned();
                    self.scan(&included, &base, depth + 1)?;
                }
                "binds" => {
                    for bind in node.children() {
                        let start = bind.span().0;
                        line += text[counted..start].matches('\n').count();
                        counted = start;
                        self.binds.push(Bind {
                            key: Hotkey::parse_niri(&bind.node_name),
                            triggers: bind.children().find_map(triggered),
                            file,
                            line,
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// An include path as niri resolves it: `~` is the home folder, anything
/// else relative to the including file's folder.
fn resolve(include: &str, base: &Path) -> PathBuf {
    match (Path::new(include).strip_prefix("~"), std::env::var_os("HOME")) {
        (Ok(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => base.join(include),
    }
}

/// A path that is equal for every spelling of the same file (`..`, symlinks),
/// whether or not it exists yet.
fn same_file_key(path: &Path) -> PathBuf {
    if let Ok(path) = fs::canonicalize(path) {
        return path;
    }
    let mut key = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                key.pop();
            }
            part => key.push(part),
        }
    }
    key
}

/// The Action a bind's action node runs through `poe-overlay trigger`.
fn triggered(action: &Spanned<Node<Span>, Span>) -> Option<Action> {
    let mut arguments = action.arguments.iter();
    match &**action.node_name {
        "spawn" => trigger_of(arguments.map(string)),
        "spawn-sh" if arguments.len() == 1 => trigger_of(string(arguments.next()?)?.split_whitespace().map(Some)),
        _ => None,
    }
}

/// The Action in exactly `poe-overlay trigger <action>`; a `None` word is an
/// argument that isn't a string.
fn trigger_of<'a>(mut words: impl Iterator<Item = Option<&'a str>>) -> Option<Action> {
    let (program, verb, name) = (words.next()??, words.next()??, words.next()??);
    let is_us = Path::new(program).file_name()?.to_str()?.starts_with("poe-overlay");
    if !is_us || verb != "trigger" || words.next().is_some() {
        return None;
    }
    <Action as clap::ValueEnum>::from_str(name, false).ok()
}

fn string(value: &Value<Span>) -> Option<&str> {
    match &*value.literal {
        Literal::String(text) => Some(text),
        _ => None,
    }
}

/// Our file: a `binds` block with the hotkeys that are free.
fn binds_file(binds: &[Planned], program: &Path) -> String {
    let mut file = String::from(
        "// Written by poe-overlay from [hotkeys] in its config.toml, and rewritten\n// when the game starts. Change the keys there, not here.\nbinds {\n",
    );
    let program = kdl_string(&program.to_string_lossy());
    for planned in binds.iter().filter(|planned| planned.fate == Fate::Bind) {
        let action = planned.action;
        let key = planned.key.to_string();
        // A bare KDL name can't start with a digit (a key like `1`).
        let key = if key.starts_with(|c: char| c.is_ascii_alphabetic()) { key } else { kdl_string(&key) };
        let _ = writeln!(
            file,
            "    {key} hotkey-overlay-title=\"PoE: {}\" {{ spawn {program} \"trigger\" \"{}\"; }}",
            action.title(),
            action.name()
        );
    }
    file.push_str("}\n");
    file
}

/// A quoted KDL string.
fn kdl_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        if matches!(c, '"' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}
