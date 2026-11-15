//! Hotkeys: the keys the player chose in `[hotkeys]` for each Action.

use std::fmt;
use std::str::FromStr;

use clap::ValueEnum;
use serde::Deserialize;

use crate::running::Action;

/// A key with modifiers, written like `Alt+R` or `Ctrl+Shift+F5`. Key names
/// are XKB keysym names, matched case-insensitively.
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

/// Display order and names; the first name is the one written out.
const MODIFIERS: [(u8, &[&str]); 4] = [(CTRL, &["Ctrl", "Control"]), (SHIFT, &["Shift"]), (ALT, &["Alt"]), (SUPER, &["Super", "Win"])];

/// The modifiers a hotkey can need on any keyboard layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Super,
}

impl Hotkey {
    /// The key's XKB keysym (the same numbers X uses).
    pub fn keysym(&self) -> u32 {
        self.keysym
    }

    /// Which of Ctrl, Shift, Alt and Super the hotkey holds.
    pub fn modifiers(&self) -> impl Iterator<Item = Modifier> {
        [(CTRL, Modifier::Ctrl), (SHIFT, Modifier::Shift), (ALT, Modifier::Alt), (SUPER, Modifier::Super)]
            .into_iter()
            .filter(|(bit, _)| self.modifiers & bit != 0)
            .map(|(_, modifier)| modifier)
    }
}

impl FromStr for Hotkey {
    type Err = String;

    fn from_str(text: &str) -> Result<Hotkey, String> {
        let mut parts = text.split('+');
        let key = parts.next_back().unwrap_or_default().trim();
        let mut modifiers = 0;
        for part in parts {
            let part = part.trim();
            let Some(&(modifier, _)) = MODIFIERS.iter().find(|(_, names)| names.iter().any(|name| name.eq_ignore_ascii_case(part))) else {
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
}

/// Looks a key name up case-insensitively, then with the `XF86` prefix (so
/// `AudioMute` works).
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
        let keys = Action::value_variants().iter().map(|&action| (action, default_key(action))).collect();
        Hotkeys { keys }
    }
}

fn default_key(action: Action) -> Option<Hotkey> {
    let text = match action {
        Action::CurrencyRates => "Alt+R",
        Action::PriceCheck => "Alt+D",
        // Rarely needed in game: no key unless the player picks one.
        Action::Settings => return None,
    };
    Some(text.parse().expect("default hotkeys are valid"))
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
