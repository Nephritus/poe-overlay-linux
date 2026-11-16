//! Chat commands sent from Trade Request cards, with a fake keyboard and
//! clipboard that log what happens in order.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use poe_overlay::hotkeys::Modifier;
use poe_overlay::poe1::chat::{CardButton, ChatClipboard, ChatError, send_chat};
use poe_overlay::poe1::keyboard::{Key, Keyboard, KeyboardError};
use poe_overlay::poe1::trade_request::Direction;

#[test]
fn each_card_button_sends_one_chat_line_for_the_character_on_the_card() {
    let lines = |direction, character| CardButton::of(direction).iter().map(|button| button.chat_line(character)).collect::<Vec<_>>();

    assert_eq!(
        lines(Direction::Incoming, "Tyrael_Buys"),
        ["/invite Tyrael_Buys", "/tradewith Tyrael_Buys", "@Tyrael_Buys Thanks, enjoy!", "/kick Tyrael_Buys", "@Tyrael_Buys Sorry, it's already sold."]
    );
    assert_eq!(lines(Direction::Outgoing, "Seller_Of_Mitts"), ["/hideout Seller_Of_Mitts", "/tradewith Seller_Of_Mitts", "@Seller_Of_Mitts Thanks, enjoy!", "/leave"]);
}

#[derive(Debug, Clone, PartialEq)]
enum Event {
    Save,
    Put(String),
    Press(Key),
    Release(Key),
    WaitPasted,
    Restore(&'static str),
}

type Log = Rc<RefCell<Vec<Event>>>;

struct FakeKeyboard {
    log: Log,
    denied: bool,
    /// A press of this key fails.
    broken: Option<Key>,
}

impl Keyboard for FakeKeyboard {
    fn ready(&mut self) -> Result<(), KeyboardError> {
        if self.denied { Err(KeyboardError::Denied) } else { Ok(()) }
    }

    fn press(&mut self, key: Key) -> Result<(), KeyboardError> {
        if self.broken == Some(key) {
            return Err(KeyboardError::Failed("device gone".into()));
        }
        self.log.borrow_mut().push(Event::Press(key));
        Ok(())
    }

    fn release(&mut self, key: Key) -> Result<(), KeyboardError> {
        self.log.borrow_mut().push(Event::Release(key));
        Ok(())
    }
}

struct FakeClipboard {
    log: Log,
    /// What the player had on the clipboard; `None`: it can't be read.
    before: Option<&'static str>,
    /// Whether the game reads the command when Ctrl+V is pressed.
    pasted: bool,
}

impl ChatClipboard for FakeClipboard {
    type Saved = &'static str;

    fn save(&mut self) -> Result<&'static str, String> {
        self.log.borrow_mut().push(Event::Save);
        self.before.ok_or_else(|| "no clipboard".to_owned())
    }

    fn put(&mut self, text: &str) -> Result<(), String> {
        self.log.borrow_mut().push(Event::Put(text.to_owned()));
        Ok(())
    }

    fn wait_pasted(&mut self, _wait: Duration) -> Result<bool, String> {
        self.log.borrow_mut().push(Event::WaitPasted);
        Ok(self.pasted)
    }

    fn restore(&mut self, saved: &'static str) -> Result<(), String> {
        self.log.borrow_mut().push(Event::Restore(saved));
        if saved == UNRESTORABLE { Err("selection lost".to_owned()) } else { Ok(()) }
    }
}

/// Clipboard content the fake can't put back.
const UNRESTORABLE: &str = "huge image";

fn run(denied: bool, broken: Option<Key>, before: Option<&'static str>, pasted: bool) -> (Result<(), ChatError>, Vec<Event>) {
    let log = Log::default();
    let mut keyboard = FakeKeyboard { log: log.clone(), denied, broken };
    let mut clipboard = FakeClipboard { log: log.clone(), before, pasted };
    let sent = send_chat(&mut keyboard, &mut clipboard, "/invite Tyrael_Buys");
    (sent, log.take())
}

const CTRL: Key = Key::Modifier(Modifier::Ctrl);

#[test]
fn command_is_pasted_into_chat_and_the_players_clipboard_put_back() {
    let (sent, events) = run(false, None, Some("my build notes"), true);

    assert!(sent.is_ok(), "{sent:?}");
    use Event::*;
    assert_eq!(
        events,
        [
            Save,
            Put("/invite Tyrael_Buys".into()),
            Press(Key::Enter),
            Release(Key::Enter),
            Press(CTRL),
            Press(Key::V),
            Release(Key::V),
            Release(CTRL),
            WaitPasted,
            Press(Key::Enter),
            Release(Key::Enter),
            Restore("my build notes"),
        ]
    );
}

#[test]
fn clipboard_is_put_back_when_a_key_press_fails() {
    let (sent, events) = run(false, Some(Key::V), Some("my build notes"), true);

    assert!(matches!(sent, Err(ChatError::Keyboard(_))), "{sent:?}");
    assert_eq!(events.last(), Some(&Event::Restore("my build notes")));
    assert!(events.contains(&Event::Release(CTRL)), "Ctrl let go: {events:?}");
    assert!(!events.contains(&Event::WaitPasted), "{events:?}");
}

#[test]
fn clipboard_not_put_back_is_reported() {
    let (sent, _) = run(false, None, Some(UNRESTORABLE), true);

    assert!(matches!(sent, Err(ChatError::Clipboard(_))), "{sent:?}");
}

#[test]
fn chat_opened_but_nothing_pasted_is_closed_empty_and_reported() {
    let (sent, events) = run(false, None, Some("my build notes"), false);

    assert!(matches!(sent, Err(ChatError::NotPasted)), "{sent:?}");
    // Enter on an empty chat line closes it without sending anything.
    let after_wait: Vec<_> = events.iter().skip_while(|event| **event != Event::WaitPasted).skip(1).cloned().collect();
    assert_eq!(after_wait, [Event::Press(Key::Enter), Event::Release(Key::Enter), Event::Restore("my build notes")]);
}

#[test]
fn nothing_is_pressed_when_the_clipboard_or_keyboard_cant_be_used() {
    let (sent, events) = run(false, None, None, true);
    assert!(matches!(sent, Err(ChatError::Clipboard(_))), "{sent:?}");
    assert_eq!(events, [Event::Save]);

    let (sent, events) = run(true, None, Some("my build notes"), true);
    assert!(matches!(sent, Err(ChatError::NoKeyboardAccess)), "{sent:?}");
    assert_eq!(events, []);
}
