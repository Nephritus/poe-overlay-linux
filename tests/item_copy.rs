//! Copying the hovered item out of the game, with a fake keyboard and
//! clipboard that log what happens in order.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use poe_overlay::hotkeys::Modifier;
use poe_overlay::poe1::item_copy::{Clipboard, CopyError, copy_item};
use poe_overlay::poe1::keyboard::{Key, Keyboard, KeyboardError};

#[derive(Debug, Clone, PartialEq)]
enum Event {
    Placeholder,
    Press(Key),
    Release(Key),
    Read,
}

type Log = Rc<RefCell<Vec<Event>>>;

struct FakeKeyboard {
    log: Log,
    denied: bool,
}

impl Keyboard for FakeKeyboard {
    fn ready(&mut self) -> Result<(), KeyboardError> {
        if self.denied { Err(KeyboardError::Denied) } else { Ok(()) }
    }

    fn press(&mut self, key: Key) -> Result<(), KeyboardError> {
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
    /// What the game puts on the clipboard; `None`: nothing was hovered.
    copied: Option<&'static str>,
}

impl Clipboard for FakeClipboard {
    fn put_placeholder(&mut self) -> Result<(), String> {
        self.log.borrow_mut().push(Event::Placeholder);
        Ok(())
    }

    fn read_next(&mut self, _wait: Duration) -> Result<Option<String>, String> {
        self.log.borrow_mut().push(Event::Read);
        Ok(self.copied.map(str::to_owned))
    }
}

fn run(held: &[Modifier], copied: Option<&'static str>, denied: bool) -> (Result<String, CopyError>, Vec<Event>) {
    let log = Log::default();
    let mut keyboard = FakeKeyboard { log: log.clone(), denied };
    let mut clipboard = FakeClipboard { log: log.clone(), copied };
    let result = copy_item(&mut keyboard, &mut clipboard, held);
    let events = log.borrow().clone();
    (result, events)
}

const ITEM: &str = "Item Class: Stackable Currency\nRarity: Currency\nChaos Orb\n--------\nStack Size: 1/10\n";

use Event::*;
use Modifier::{Alt, Ctrl, Shift, Super};

const fn press(modifier: Modifier) -> Event {
    Press(Key::Modifier(modifier))
}

const fn release(modifier: Modifier) -> Event {
    Release(Key::Modifier(modifier))
}

const CTRL_ALT_C: [Event; 6] = [press(Ctrl), press(Alt), Press(Key::C), Release(Key::C), release(Alt), release(Ctrl)];

#[test]
fn placeholder_then_ctrl_alt_c_then_one_read() {
    // The placeholder makes Proton 11 hand the copy on to other apps.
    let (result, events) = run(&[Alt], Some(ITEM), false);

    assert_eq!(result.unwrap(), ITEM);
    let expected: Vec<Event> = [Placeholder].into_iter().chain(CTRL_ALT_C).chain([Read]).collect();
    assert_eq!(events, expected, "Alt is pressed anyway");
}

#[test]
fn other_held_modifiers_are_let_go_before_the_copy() {
    // A Shift+Super+D hotkey: the game would see Ctrl+Alt+Shift+Super+C.
    let (_, events) = run(&[Shift, Super, Ctrl], Some(ITEM), false);

    let expected: Vec<Event> =
        [Placeholder, press(Shift), release(Shift), press(Super), release(Super)].into_iter().chain(CTRL_ALT_C).chain([Read]).collect();
    assert_eq!(events, expected);
}

#[test]
fn nothing_copied_is_one_read_and_a_hint() {
    let (result, events) = run(&[], None, false);

    let err = result.unwrap_err();
    assert!(matches!(err, CopyError::NothingCopied));
    assert!(err.to_string().contains("hover over an item"), "{err}");
    assert_eq!(events.iter().filter(|e| **e == Read).count(), 1);
}

#[test]
fn without_uinput_access_the_udev_rule_is_shown_and_nothing_is_touched() {
    let (result, events) = run(&[], Some(ITEM), true);

    let err = result.unwrap_err().to_string();
    assert!(err.contains(r#"KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput""#), "{err}");
    assert!(events.is_empty(), "no clipboard change, no keys: {events:?}");
}
