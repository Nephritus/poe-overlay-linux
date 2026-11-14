//! Hotkeys: the keys the player chose in `[hotkeys]` for each Action, and
//! putting them into the compositor's own config, since on Wayland only the
//! compositor sees keys pressed in other windows.

use std::collections::BTreeSet;
use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::running::Action;

pub mod niri;

use niri::Niri;

/// A key with modifiers, written like `Alt+R` or `Ctrl+Shift+F5`. Key names
/// are XKB keysym names, matched case-insensitively as niri does.
#[derive(Debug, Clone)]
pub struct Hotkey {
    modifiers: u8,
    keysym: u32,
    /// The key name as the player wrote it.
    key: Box<str>,
}

const CTRL: u8 = 1;
const SHIFT: u8 = 1 << 1;
const ALT: u8 = 1 << 2;
const SUPER: u8 = 1 << 3;
const ISO_LEVEL3_SHIFT: u8 = 1 << 4;
const ISO_LEVEL5_SHIFT: u8 = 1 << 5;

/// Display order and names; the first name is the one written out.
const MODIFIERS: [(u8, &[&str]); 6] = [
    (CTRL, &["Ctrl", "Control"]),
    (SHIFT, &["Shift"]),
    (ALT, &["Alt"]),
    (SUPER, &["Super", "Win"]),
    (ISO_LEVEL3_SHIFT, &["ISO_Level3_Shift", "Mod5"]),
    (ISO_LEVEL5_SHIFT, &["ISO_Level5_Shift", "Mod3"]),
];

impl Hotkey {
    /// Parses a key as niri writes it: also `Mod` (Super in a normal session)
    /// and the ISO level shifts.
    pub(crate) fn parse_niri(text: &str) -> Option<Hotkey> {
        parse(text, true).ok()
    }
}

impl PartialEq for Hotkey {
    fn eq(&self, other: &Hotkey) -> bool {
        self.modifiers == other.modifiers && self.keysym == other.keysym
    }
}

impl Eq for Hotkey {}

impl FromStr for Hotkey {
    type Err = String;

    fn from_str(text: &str) -> Result<Hotkey, String> {
        parse(text, false)
    }
}

fn parse(text: &str, niri: bool) -> Result<Hotkey, String> {
    let mut parts = text.split('+');
    let key = parts.next_back().unwrap_or_default().trim();
    let mut modifiers = 0;
    for part in parts {
        let part = part.trim();
        let modifier = if niri && part.eq_ignore_ascii_case("mod") {
            Some(SUPER)
        } else {
            MODIFIERS
                .iter()
                .find(|(_, names)| names.iter().any(|name| name.eq_ignore_ascii_case(part)))
                .map(|&(modifier, _)| modifier)
        };
        let Some(modifier) = modifier else {
            return Err(format!("unknown modifier `{part}` in `{text}` (use Ctrl, Shift, Alt or Super)"));
        };
        modifiers |= modifier;
    }
    if key.is_empty() {
        return Err(format!("no key after the modifiers in `{text}`"));
    }
    let Some(keysym) = keysym(key) else {
        return Err(format!("unknown key `{key}` in `{text}` (use an XKB key name such as R, F5 or Return)"));
    };
    Ok(Hotkey { modifiers, keysym, key: key.into() })
}

/// Looks a key name up like niri: case-insensitively, then with the `XF86`
/// prefix (so `AudioMute` works).
fn keysym(name: &str) -> Option<u32> {
    use xkbcommon::xkb::{KEYSYM_CASE_INSENSITIVE, keysym_from_name, keysyms::KEY_NoSymbol};
    if name.contains('\0') {
        return None;
    }
    let found = |name: &str| Some(keysym_from_name(name, KEYSYM_CASE_INSENSITIVE).raw()).filter(|&keysym| keysym != KEY_NoSymbol);
    found(name).or_else(|| found(&format!("XF86{name}")))
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (modifier, names) in MODIFIERS {
            if self.modifiers & modifier != 0 {
                write!(f, "{}+", names[0])?;
            }
        }
        f.write_str(&self.key)
    }
}

/// The hotkey for each Action; `""` in `[hotkeys]` leaves an Action without
/// one, and an Action not mentioned keeps its default.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "toml::Table")]
pub struct Hotkeys {
    keys: Vec<(Action, Option<Hotkey>)>,
}

impl Default for Hotkeys {
    fn default() -> Hotkeys {
        let keys = Action::value_variants().iter().map(|&action| (action, Some(default_key(action)))).collect();
        Hotkeys { keys }
    }
}

fn default_key(action: Action) -> Hotkey {
    let text = match action {
        Action::CurrencyRates => "Alt+R",
    };
    text.parse().expect("default hotkeys are valid")
}

impl Hotkeys {
    pub fn get(&self, action: Action) -> Option<&Hotkey> {
        self.keys.iter().find(|(a, _)| *a == action).and_then(|(_, key)| key.as_ref())
    }

    /// Actions that have a hotkey, in Action order.
    pub fn iter(&self) -> impl Iterator<Item = (Action, &Hotkey)> {
        self.keys.iter().filter_map(|(action, key)| Some((*action, key.as_ref()?)))
    }
}

impl TryFrom<toml::Table> for Hotkeys {
    type Error = String;

    fn try_from(table: toml::Table) -> Result<Hotkeys, String> {
        let mut hotkeys = Hotkeys::default();
        for (name, value) in table {
            let Ok(action) = Action::from_str(&name, false) else {
                let valid: Vec<_> = Action::value_variants().iter().map(|a| a.name()).collect();
                return Err(format!("unknown Action `{name}` (valid: {})", valid.join(", ")));
            };
            let Some(text) = value.as_str() else {
                return Err(format!("the hotkey for `{name}` must be a string like \"Alt+R\""));
            };
            let key = match text.trim() {
                "" => None,
                text => Some(text.parse()?),
            };
            let slot = hotkeys.keys.iter_mut().find(|(a, _)| *a == action).expect("every Action has a slot");
            slot.1 = key;
        }
        Ok(hotkeys)
    }
}

/// What setting up the hotkeys does in a compositor's config: one entry per
/// Action that has a hotkey.
#[derive(Debug)]
pub struct Plan {
    pub binds: Vec<Planned>,
    /// Our file is already included from the compositor's config.
    pub installed: bool,
    /// Installed, and our file already holds exactly these binds.
    pub up_to_date: bool,
    /// The text of our file.
    file: String,
}

impl Plan {
    /// The hotkeys that get bound.
    pub fn to_bind(&self) -> impl Iterator<Item = &Planned> {
        self.binds.iter().filter(|planned| planned.fate == Fate::Bind)
    }

    /// The hotkeys whose key the player's own binds already use.
    pub fn taken(&self) -> impl Iterator<Item = (&Planned, &Location)> {
        self.binds.iter().filter_map(|planned| match &planned.fate {
            Fate::Taken(at) => Some((planned, at)),
            _ => None,
        })
    }

    /// One line per taken key: where it's used and what to do; empty if none.
    pub fn taken_text(&self) -> String {
        let mut text = String::new();
        for (planned, at) in self.taken() {
            if !text.is_empty() {
                text.push('\n');
            }
            let (key, title) = (&planned.key, planned.action.title());
            let _ = write!(text, "{key} is already used ({at}), so {title} has no hotkey; choose another key under [hotkeys] in ");
            match crate::config::path() {
                Some(path) => text.push_str(&path.to_string_lossy()),
                None => text.push_str("poe-overlay's config.toml"),
            }
            text.push('.');
        }
        text
    }
}

#[derive(Debug)]
pub struct Planned {
    pub action: Action,
    pub key: Hotkey,
    pub fate: Fate,
}

#[derive(Debug, PartialEq)]
pub enum Fate {
    /// The key is free and gets bound to the Action.
    Bind,
    /// The player already bound some key to this Action themselves.
    ByHand(Location),
    /// The key does something else in the player's config.
    Taken(Location),
}

/// A line in a compositor config file.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    /// Relative to the compositor's config folder when inside it.
    pub file: PathBuf,
    /// 1-based.
    pub line: usize,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} line {}", self.file.display(), self.line)
    }
}

/// Where hotkeys get bound.
pub enum Compositor {
    Niri(Niri),
    /// One we can't set up: the player binds the trigger commands by hand.
    Unknown,
}

impl Compositor {
    /// The compositor of the session we run in.
    pub fn detect() -> Compositor {
        match Niri::from_env() {
            Some(niri) => Compositor::Niri(niri),
            None => Compositor::Unknown,
        }
    }

    /// Names the compositor in saved answers.
    fn key(&self) -> &'static str {
        match self {
            Compositor::Niri(_) => "niri",
            Compositor::Unknown => "other",
        }
    }
}

/// The player's answers to the question at launch, per compositor (a "not
/// now" to the by-hand commands mustn't hide the niri question), kept in a
/// small TOML file.
pub struct Answers {
    path: PathBuf,
}

#[derive(Default, Deserialize, Serialize)]
struct AnswersFile {
    #[serde(default)]
    declined: BTreeSet<String>,
}

impl Answers {
    pub fn at(path: impl Into<PathBuf>) -> Answers {
        Answers { path: path.into() }
    }

    /// `$XDG_STATE_HOME/poe-overlay/hotkeys.toml`.
    pub fn default_path() -> PathBuf {
        crate::xdg::state_home().unwrap_or_else(std::env::temp_dir).join("poe-overlay/hotkeys.toml")
    }

    /// An unreadable file counts as no answer: asking again is harmless.
    fn read(&self) -> AnswersFile {
        fs::read_to_string(&self.path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
    }

    fn declined(&self, compositor: &Compositor) -> bool {
        self.read().declined.contains(compositor.key())
    }

    fn set_declined(&self, compositor: &Compositor, declined: bool) -> io::Result<()> {
        let mut file = self.read();
        let changed = if declined { file.declined.insert(compositor.key().to_owned()) } else { file.declined.remove(compositor.key()) };
        if !changed {
            return Ok(());
        }
        let text = toml::to_string(&file).map_err(io::Error::other)?;
        crate::files::write_atomic(&self.path, &text)
    }
}

/// What the overlay shows about hotkeys when the game starts.
#[derive(Debug)]
pub enum LaunchPrompt {
    Nothing,
    /// Not set up yet: ask to bind the free keys and name the taken ones.
    Ask(Plan),
    /// Set up, but some keys are now taken by the player's own binds; they
    /// were left out of ours.
    Conflicts(Plan),
    /// No compositor we can set up: show the commands to bind by hand.
    Manual,
    /// The compositor's config couldn't be read or written.
    Failed(String),
}

/// Sets up the player's hotkeys in their compositor, asking first.
pub struct HotkeySetup {
    compositor: Compositor,
    hotkeys: Hotkeys,
    /// What the binds run.
    program: PathBuf,
    answers: Answers,
}

impl HotkeySetup {
    pub fn new(compositor: Compositor, hotkeys: Hotkeys, program: PathBuf, answers: Answers) -> HotkeySetup {
        HotkeySetup { compositor, hotkeys, program, answers }
    }

    /// For the session we run in: its compositor, this program, the saved
    /// answer.
    pub fn for_session(hotkeys: Hotkeys) -> io::Result<HotkeySetup> {
        Ok(HotkeySetup::new(Compositor::detect(), hotkeys, program()?, Answers::at(Answers::default_path())))
    }

    /// Decides what to ask when the game starts. Hotkeys already set up are
    /// brought in line with `[hotkeys]` without asking.
    pub fn at_launch(&self) -> LaunchPrompt {
        let Compositor::Niri(niri) = &self.compositor else {
            return if self.answers.declined(&self.compositor) || self.hotkeys.iter().next().is_none() { LaunchPrompt::Nothing } else { LaunchPrompt::Manual };
        };
        let plan = match niri.plan(&self.hotkeys, &self.program) {
            Ok(plan) => plan,
            Err(err) => return LaunchPrompt::Failed(err),
        };
        let taken = plan.taken().next().is_some();
        if plan.installed {
            if !plan.up_to_date
                && let Err(err) = niri.apply(&plan)
            {
                return LaunchPrompt::Failed(format!("cannot update the hotkeys: {err}"));
            }
            return if taken { LaunchPrompt::Conflicts(plan) } else { LaunchPrompt::Nothing };
        }
        let to_bind = plan.to_bind().next().is_some();
        if self.answers.declined(&self.compositor) || !(to_bind || taken) {
            return LaunchPrompt::Nothing;
        }
        LaunchPrompt::Ask(plan)
    }

    /// Binds the free keys (the player said yes, or ran `setup hotkeys`).
    pub fn set_up(&self) -> Result<Plan, String> {
        let Compositor::Niri(niri) = &self.compositor else {
            return Err(self.manual_instructions());
        };
        let plan = niri.plan(&self.hotkeys, &self.program)?;
        niri.apply(&plan).map_err(|err| format!("cannot write the niri config: {err}"))?;
        // Saying yes once overrides an earlier "not now".
        if let Err(err) = self.answers.set_declined(&self.compositor, false) {
            eprintln!("poe-overlay: cannot save the hotkey answer: {err}");
        }
        Ok(plan)
    }

    /// "Not now": don't ask again in this compositor.
    pub fn decline(&self) -> io::Result<()> {
        self.answers.set_declined(&self.compositor, true)
    }

    /// Takes our binds out again and stops the question at launch; returns
    /// whether there was anything to take out.
    pub fn remove(&self) -> Result<bool, String> {
        let removed = match &self.compositor {
            Compositor::Niri(niri) => niri.remove()?,
            Compositor::Unknown => false,
        };
        self.decline().map_err(|err| format!("cannot save the hotkey answer: {err}"))?;
        Ok(removed)
    }

    /// How to bind the hotkeys by hand, for compositors we can't set up.
    pub fn manual_instructions(&self) -> String {
        let mut text = String::from("Bind these in your compositor's config:");
        for (action, key) in self.hotkeys.iter() {
            let _ = write!(text, "\n  {key}: poe-overlay trigger {}  ({})", action.name(), action.title());
        }
        text
    }
}

/// What hotkey binds should run: this program where it's installed (an
/// AppImage's own file, not its temporary mount).
pub fn program() -> io::Result<PathBuf> {
    match std::env::var_os("APPIMAGE") {
        Some(path) => Ok(path.into()),
        None => std::env::current_exe(),
    }
}

/// `poe-overlay setup hotkeys [--remove]`.
pub fn run_setup(remove: bool) -> ExitCode {
    let fail = |message: &str| {
        eprintln!("poe-overlay: {message}");
        ExitCode::FAILURE
    };
    // Removing doesn't need the keys, so a broken config.toml can't block it.
    let hotkeys = match crate::config::Config::load() {
        Ok(config) => config.hotkeys,
        Err(_) if remove => Hotkeys::default(),
        Err(err) => return fail(&err),
    };
    let setup = match HotkeySetup::for_session(hotkeys) {
        Ok(setup) => setup,
        Err(err) => return fail(&format!("cannot find this program's path: {err}")),
    };
    if remove {
        return match setup.remove() {
            Ok(true) => {
                println!("Removed poe-overlay's hotkeys from the niri config; the game won't ask about them again.");
                ExitCode::SUCCESS
            }
            Ok(false) => {
                println!("No poe-overlay hotkeys to remove; the game won't ask about them again.");
                ExitCode::SUCCESS
            }
            Err(err) => fail(&err),
        };
    }
    if let Compositor::Unknown = setup.compositor {
        return fail(&format!("poe-overlay can set up hotkeys only in niri for now. {}", setup.manual_instructions()));
    }
    let plan = match setup.set_up() {
        Ok(plan) => plan,
        Err(err) => return fail(&err),
    };
    for planned in &plan.binds {
        let (key, title) = (&planned.key, planned.action.title());
        match &planned.fate {
            Fate::Bind => println!("{key}: {title}"),
            Fate::ByHand(at) => println!("{title}: already bound by you ({at}), left as is"),
            Fate::Taken(_) => {}
        }
    }
    if plan.binds.is_empty() {
        println!("No hotkeys in [hotkeys]; nothing bound.");
    }
    let taken = plan.taken_text();
    if taken.is_empty() {
        return ExitCode::SUCCESS;
    }
    fail(&taken)
}
