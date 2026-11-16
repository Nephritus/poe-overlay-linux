//! Chat commands from Trade Request card buttons. Each click sends exactly
//! one chat line (GGG's one action per input): the line goes on the
//! clipboard and is pasted into chat (Enter, Ctrl+V, Enter), which works with
//! any keyboard layout; the player's clipboard is put back afterwards.

use std::fmt;
use std::time::Duration;

use super::keyboard::{Key, Keyboard, KeyboardError, Modifier, UINPUT_RULE};
use super::trade_request::Direction;

/// How long the game gets to read the pasted line.
const PASTE_WAIT: Duration = Duration::from_millis(1500);

/// A card button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardAction {
    Invite,
    Trade,
    Thank,
    Kick,
    Sold,
    VisitHideout,
    LeaveParty,
}

impl CardAction {
    pub const ALL: [CardAction; 7] = [
        CardAction::Invite,
        CardAction::Trade,
        CardAction::Thank,
        CardAction::Kick,
        CardAction::Sold,
        CardAction::VisitHideout,
        CardAction::LeaveParty,
    ];

    /// A card's buttons: a buyer is invited to the player's hideout, the
    /// player visits a seller's.
    pub fn of(direction: Direction) -> &'static [CardAction] {
        match direction {
            Direction::Incoming => &[CardAction::Invite, CardAction::Trade, CardAction::Thank, CardAction::Kick, CardAction::Sold],
            Direction::Outgoing => &[CardAction::VisitHideout, CardAction::Trade, CardAction::Thank, CardAction::LeaveParty],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CardAction::Invite => "Invite",
            CardAction::Trade => "Trade",
            CardAction::Thank => "Thank",
            CardAction::Kick => "Kick",
            CardAction::Sold => "Sold",
            CardAction::VisitHideout => "Hideout",
            CardAction::LeaveParty => "Leave",
        }
    }
}

/// The chat line `action` sends for the card's `character`.
pub fn command(action: CardAction, character: &str) -> String {
    match action {
        CardAction::Invite => format!("/invite {character}"),
        CardAction::Trade => format!("/tradewith {character}"),
        CardAction::Thank => format!("@{character} Thanks, enjoy!"),
        CardAction::Kick => format!("/kick {character}"),
        CardAction::Sold => format!("@{character} Sorry, it's already sold."),
        CardAction::VisitHideout => format!("/hideout {character}"),
        CardAction::LeaveParty => "/leave".to_owned(),
    }
}

/// The compositor's clipboard, as chat lines need it.
pub trait ChatClipboard {
    /// What the player had on the clipboard.
    type Saved;
    fn save(&mut self) -> Result<Self::Saved, String>;
    /// Puts `text` on the clipboard.
    fn offer(&mut self, text: &str) -> Result<(), String>;
    /// Waits up to `wait` for a program (the game) to read what was offered.
    fn wait_pasted(&mut self, wait: Duration) -> Result<bool, String>;
    fn restore(&mut self, saved: Self::Saved) -> Result<(), String>;
}

#[derive(Debug)]
pub enum ChatError {
    NoKeyboardAccess,
    Keyboard(String),
    Clipboard(String),
    /// The game didn't read the line: its chat stays empty.
    NotPasted,
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChatError::NoKeyboardAccess => write!(
                f,
                "Can't type in chat: /dev/uinput isn't writable. Put this line in /etc/udev/rules.d/60-poe-overlay-uinput.rules, \
                 then run `sudo udevadm control --reload && sudo udevadm trigger`:\n{UINPUT_RULE}"
            ),
            ChatError::Keyboard(err) => write!(f, "Couldn't press the chat keys: {err}"),
            ChatError::Clipboard(err) => write!(f, "Couldn't use the clipboard: {err}"),
            ChatError::NotPasted => write!(f, "The game didn't paste the message: click again with the game in front."),
        }
    }
}

impl From<KeyboardError> for ChatError {
    fn from(err: KeyboardError) -> ChatError {
        match err {
            KeyboardError::Denied => ChatError::NoKeyboardAccess,
            KeyboardError::Failed(err) => ChatError::Keyboard(err),
        }
    }
}

/// Sends `text` as one chat line, then puts the player's clipboard back,
/// whatever happened in between.
pub fn send_chat<C: ChatClipboard>(keyboard: &mut impl Keyboard, clipboard: &mut C, text: &str) -> Result<(), ChatError> {
    keyboard.ready()?;
    let saved = clipboard.save().map_err(ChatError::Clipboard)?;
    let sent = paste_and_send(keyboard, clipboard, text);
    let restored = clipboard.restore(saved).map_err(ChatError::Clipboard);
    sent.and(restored)
}

fn paste_and_send(keyboard: &mut impl Keyboard, clipboard: &mut impl ChatClipboard, text: &str) -> Result<(), ChatError> {
    clipboard.offer(text).map_err(ChatError::Clipboard)?;
    keyboard.tap(Key::Enter)?;
    let ctrl = Key::Modifier(Modifier::Ctrl);
    keyboard.press(ctrl)?;
    let pasting = keyboard.tap(Key::V);
    // Let go of Ctrl even if V failed, or the game sees it held.
    keyboard.release(ctrl)?;
    pasting?;
    let pasted = clipboard.wait_pasted(PASTE_WAIT);
    // Sends the line; on an empty one, just closes the chat.
    keyboard.tap(Key::Enter)?;
    match pasted {
        Ok(true) => Ok(()),
        Ok(false) => Err(ChatError::NotPasted),
        Err(err) => Err(ChatError::Clipboard(err)),
    }
}
