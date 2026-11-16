//! Keys pressed for the player in the game: the Price Check copy and chat
//! commands from Trade Request cards.

pub use crate::hotkeys::Modifier;

/// Keys the overlay presses, by what they do rather than where they sit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Modifier(Modifier),
    C,
    V,
    Enter,
}

/// Key presses the game sees as if typed.
pub trait Keyboard {
    /// Whether keys can be pressed at all.
    fn ready(&mut self) -> Result<(), KeyboardError>;
    fn press(&mut self, key: Key) -> Result<(), KeyboardError>;
    fn release(&mut self, key: Key) -> Result<(), KeyboardError>;

    /// Presses and lets go of `key`.
    fn tap(&mut self, key: Key) -> Result<(), KeyboardError> {
        self.press(key)?;
        self.release(key)
    }
}

#[derive(Debug, Clone)]
pub enum KeyboardError {
    /// The virtual keyboard device may not be opened.
    Denied,
    Failed(String),
}

/// The udev rule Steam ships (steam-devices) that lets the logged-in user
/// create virtual input devices.
pub const UINPUT_RULE: &str = r#"KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput""#;
