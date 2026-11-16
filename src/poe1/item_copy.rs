//! Copying the hovered item's text out of the game: press Ctrl+Alt+C on a
//! virtual keyboard, then read what the game put on the clipboard, once
//! (GGG's rule for tools: one clipboard read per key press).

use std::fmt;
use std::time::Duration;

use super::keyboard::{Key, Keyboard, KeyboardError, UINPUT_RULE};
use crate::hotkeys::Modifier;

/// How long the game gets to put the item on the clipboard.
const COPY_WAIT: Duration = Duration::from_millis(1500);

/// The compositor's clipboard, used without keyboard focus.
pub trait Clipboard {
    /// Puts placeholder text on the clipboard, so the game's copy is a change
    /// even when it copies the same item again.
    fn put_placeholder(&mut self) -> Result<(), String>;
    /// Waits up to `wait` for another program to put something on the
    /// clipboard and reads its text: the one read. `None` if nothing came.
    fn read_next(&mut self, wait: Duration) -> Result<Option<String>, String>;
}

#[derive(Debug)]
pub enum CopyError {
    NoKeyboardAccess,
    Keyboard(String),
    Clipboard(String),
    NothingCopied,
}

impl fmt::Display for CopyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CopyError::NoKeyboardAccess => write!(
                f,
                "Price Check can't press Ctrl+Alt+C for you: /dev/uinput isn't writable. Put this line in \
                 /etc/udev/rules.d/60-poe-overlay-uinput.rules, then run `sudo udevadm control --reload && sudo udevadm trigger` \
                 (or install your distribution's steam-devices package):\n{UINPUT_RULE}"
            ),
            CopyError::Keyboard(err) => write!(f, "Couldn't press Ctrl+Alt+C: {err}"),
            CopyError::Clipboard(err) => write!(f, "Couldn't read the clipboard: {err}"),
            CopyError::NothingCopied => write!(f, "The game copied nothing: hover over an item and press the hotkey again."),
        }
    }
}

impl From<KeyboardError> for CopyError {
    fn from(err: KeyboardError) -> CopyError {
        match err {
            KeyboardError::Denied => CopyError::NoKeyboardAccess,
            KeyboardError::Failed(err) => CopyError::Keyboard(err),
        }
    }
}

/// Copies the hovered item. `held` are the hotkey's modifiers, which the
/// player may still hold: those the copy doesn't press itself are let go
/// first (a press and release on the virtual keyboard), so the game sees
/// exactly Ctrl+Alt+C.
pub fn copy_item(keyboard: &mut impl Keyboard, clipboard: &mut impl Clipboard, held: &[Modifier]) -> Result<String, CopyError> {
    keyboard.ready()?;
    clipboard.put_placeholder().map_err(CopyError::Clipboard)?;
    for &modifier in held.iter().filter(|modifier| matches!(modifier, Modifier::Shift | Modifier::Super)) {
        keyboard.press(Key::Modifier(modifier))?;
        keyboard.release(Key::Modifier(modifier))?;
    }
    const COPY: [Key; 3] = [Key::Modifier(Modifier::Ctrl), Key::Modifier(Modifier::Alt), Key::C];
    for key in COPY {
        keyboard.press(key)?;
    }
    for key in COPY.into_iter().rev() {
        keyboard.release(key)?;
    }
    clipboard.read_next(COPY_WAIT).map_err(CopyError::Clipboard)?.ok_or(CopyError::NothingCopied)
}
