//! A virtual keyboard (Linux uinput): its key presses reach the focused
//! window through the compositor like typed ones, on any compositor and for
//! X11 and Wayland games alike.

use std::io;
use std::time::Duration;

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode};

use crate::poe1::item_copy::{Key, Keyboard, KeyboardError, Modifier};

/// Between key events, so the game sees each one.
const KEY_GAP: Duration = Duration::from_millis(5);

pub struct VirtualKeyboard {
    device: Result<VirtualDevice, KeyboardError>,
    /// The key that types `c` in the player's layout.
    c: KeyCode,
}

impl VirtualKeyboard {
    /// Creates the device now: the compositor takes a moment to add a new
    /// keyboard, and the first Price Check shouldn't lose its keys to that.
    /// `c` is the evdev code of the key that types `c` in the player's
    /// layout (AZERTY and Dvorak move it).
    pub fn new(c: u16) -> VirtualKeyboard {
        let c = KeyCode(c);
        let device = create(c).map_err(|err| match err.kind() {
            io::ErrorKind::PermissionDenied | io::ErrorKind::NotFound => KeyboardError::Denied,
            _ => KeyboardError::Failed(format!("cannot create a virtual keyboard: {err}")),
        });
        if let Err(KeyboardError::Failed(err)) = &device {
            eprintln!("poe-overlay: {err}");
        }
        VirtualKeyboard { device, c }
    }

    fn code(&self, key: Key) -> KeyCode {
        match key {
            Key::Modifier(Modifier::Ctrl) => KeyCode::KEY_LEFTCTRL,
            Key::Modifier(Modifier::Shift) => KeyCode::KEY_LEFTSHIFT,
            Key::Modifier(Modifier::Alt) => KeyCode::KEY_LEFTALT,
            Key::Modifier(Modifier::Super) => KeyCode::KEY_LEFTMETA,
            Key::C => self.c,
        }
    }

    fn emit(&mut self, key: Key, value: i32) -> Result<(), KeyboardError> {
        let code = self.code(key);
        let device = self.device.as_mut().map_err(|err| err.clone())?;
        device
            .emit(&[InputEvent::new(EventType::KEY.0, code.code(), value)])
            .map_err(|err| KeyboardError::Failed(format!("virtual keyboard: {err}")))?;
        std::thread::sleep(KEY_GAP);
        Ok(())
    }
}

fn create(c: KeyCode) -> io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for key in [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_LEFTALT, KeyCode::KEY_LEFTMETA, c] {
        keys.insert(key);
    }
    VirtualDevice::builder()?.name("poe-overlay keyboard").with_keys(&keys)?.build()
}

impl Keyboard for VirtualKeyboard {
    fn ready(&mut self) -> Result<(), KeyboardError> {
        self.device.as_ref().map(|_| ()).map_err(KeyboardError::clone)
    }

    fn press(&mut self, key: Key) -> Result<(), KeyboardError> {
        self.emit(key, 1)
    }

    fn release(&mut self, key: Key) -> Result<(), KeyboardError> {
        self.emit(key, 0)
    }
}
