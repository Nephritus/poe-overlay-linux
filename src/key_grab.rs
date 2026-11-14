//! Hotkeys caught by grabbing their keys in Xwayland while the game is focused.
//!
//! Under Proton the game is an X11 window, so Xwayland receives its keys.
//! A passive grab on the root window catches a key while any X11 window has
//! focus; holding grabs only while the game is focused leaves the keys to
//! every other window.

use std::sync::{Arc, Mutex, PoisonError, mpsc};

use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{self, ConnectionExt as _, GrabMode, ModMask, Window};
use x11rb::protocol::{ErrorKind, Event};
use x11rb::rust_connection::RustConnection;

use crate::hotkeys::{Hotkey, Hotkeys, Modifier};
use crate::running::Action;

// X core modifier mask bits; the Mod1–Mod5 ones depend on the modifier map.
const SHIFT_MASK: u16 = 1;
const LOCK_MASK: u16 = 1 << 1;
const CONTROL_MASK: u16 = 1 << 2;
const MOD1_MASK: u16 = 1 << 3;
const MOD2_MASK: u16 = 1 << 4;
const MOD4_MASK: u16 = 1 << 6;

// Keysyms of the keys that make up the modifiers.
const ALT_KEYSYMS: [u32; 2] = [0xffe9, 0xffea];
const SUPER_KEYSYMS: [u32; 2] = [0xffeb, 0xffec];
const NUM_LOCK_KEYSYM: u32 = 0xff7f;

/// The X server's keyboard: which keysyms each keycode types, and which
/// keycodes make up each of the 8 modifiers.
pub struct Keymap {
    min_keycode: u8,
    keysyms_per_keycode: usize,
    keysyms: Vec<u32>,
    modifier_keycodes: Vec<u8>,
}

impl Keymap {
    /// From the replies to `GetKeyboardMapping` (starting at the server's
    /// minimum keycode) and `GetModifierMapping`.
    pub fn new(min_keycode: u8, keysyms_per_keycode: u8, keysyms: Vec<u32>, modifier_keycodes: Vec<u8>) -> Keymap {
        Keymap { min_keycode, keysyms_per_keycode: usize::from(keysyms_per_keycode.max(1)), keysyms, modifier_keycodes }
    }

    /// The keycodes that type `keysym` without Shift in the first layout, or
    /// else in the second. A key elsewhere would be grabbed on a key that
    /// types something else.
    fn keycodes(&self, keysym: u32) -> Vec<u8> {
        // Core keymap columns: first layout plain and shifted, then the second's.
        for column in [0, 2] {
            let keycodes: Vec<u8> = self
                .keysyms
                .chunks(self.keysyms_per_keycode)
                .zip(self.min_keycode..=u8::MAX)
                .filter(|(syms, _)| syms.get(column) == Some(&keysym))
                .map(|(_, keycode)| keycode)
                .collect();
            if !keycodes.is_empty() {
                return keycodes;
            }
        }
        Vec::new()
    }

    /// The keysyms `keycode` types, one per level.
    fn keysyms_of(&self, keycode: u8) -> &[u32] {
        let Some(index) = keycode.checked_sub(self.min_keycode) else { return &[] };
        let start = usize::from(index) * self.keysyms_per_keycode;
        self.keysyms.get(start..start + self.keysyms_per_keycode).unwrap_or(&[])
    }

    /// The mask bit of the modifier holding a key that types one of
    /// `keysyms`, or `fallback` if none does.
    fn modifier_of(&self, keysyms: &[u32], fallback: u16) -> u16 {
        let per_modifier = (self.modifier_keycodes.len() / 8).max(1);
        self.modifier_keycodes
            .chunks(per_modifier)
            .position(|row| row.iter().any(|&keycode| self.keysyms_of(keycode).iter().any(|sym| keysyms.contains(sym))))
            .map_or(fallback, |index| 1 << index)
    }
}

/// One passive key grab: a keycode with exactly these modifiers held.
#[derive(Debug, Clone, Copy)]
pub struct Grab {
    pub action: Action,
    pub keycode: u8,
    pub modifiers: u16,
}

/// The grabs that catch the player's hotkeys on one keyboard layout, and
/// which Action the grabbed key events stand for. The default grabs nothing.
#[derive(Default)]
pub struct GrabPlan {
    keys: Vec<HotkeyKeys>,
    /// CapsLock and NumLock: held or not, they mustn't stop a hotkey.
    locks: u16,
    unmapped: Vec<(Action, Hotkey)>,
    /// The hotkey whose key is down, and that key.
    held: Option<(Action, u8)>,
}

/// A hotkey as X sees it: the keys that type its keysym, with modifiers.
struct HotkeyKeys {
    action: Action,
    keycodes: Vec<u8>,
    modifiers: u16,
}

impl GrabPlan {
    pub fn new(hotkeys: &Hotkeys, keymap: &Keymap) -> GrabPlan {
        let alt = keymap.modifier_of(&ALT_KEYSYMS, MOD1_MASK);
        let super_ = keymap.modifier_of(&SUPER_KEYSYMS, MOD4_MASK);
        let mut keys = Vec::new();
        let mut unmapped = Vec::new();
        for (action, hotkey) in hotkeys.iter() {
            let keycodes = keymap.keycodes(hotkey.keysym());
            if keycodes.is_empty() {
                unmapped.push((action, hotkey.clone()));
                continue;
            }
            let modifiers = hotkey.modifiers().fold(0, |mask, modifier| {
                mask | match modifier {
                    Modifier::Ctrl => CONTROL_MASK,
                    Modifier::Shift => SHIFT_MASK,
                    Modifier::Alt => alt,
                    Modifier::Super => super_,
                }
            });
            keys.push(HotkeyKeys { action, keycodes, modifiers });
        }
        let locks = LOCK_MASK | keymap.modifier_of(&[NUM_LOCK_KEYSYM], MOD2_MASK);
        GrabPlan { keys, locks, unmapped, held: None }
    }

    /// Every grab to request: each hotkey with each combination of locks.
    pub fn grabs(&self) -> impl Iterator<Item = Grab> {
        let locks = self.locks;
        let lock_sets = [0, LOCK_MASK, locks & !LOCK_MASK, locks];
        self.keys.iter().flat_map(move |&HotkeyKeys { action, ref keycodes, modifiers }| {
            keycodes.iter().flat_map(move |&keycode| lock_sets.into_iter().map(move |lock| Grab { action, keycode, modifiers: modifiers | lock }))
        })
    }

    /// Hotkeys whose key isn't on the keyboard layout.
    pub fn unmapped(&self) -> impl Iterator<Item = (Action, &Hotkey)> {
        self.unmapped.iter().map(|(action, key)| (*action, key))
    }

    /// A grabbed key went down with the modifier `state` of the X event.
    pub fn press(&mut self, keycode: u8, state: u16) {
        // Only the 8 modifiers; the rest are pointer buttons.
        let modifiers = state & 0xff & !self.locks;
        let hotkey = self.keys.iter().find(|key| key.modifiers == modifiers && key.keycodes.contains(&keycode));
        if let Some(hotkey) = hotkey {
            self.held = Some((hotkey.action, keycode));
        }
    }

    /// A key went up: the Action of the hotkey it ends, if any. Actions run
    /// on release because while the key is down X sends every key to us,
    /// including the ones an Action presses in the game.
    pub fn release(&mut self, keycode: u8) -> Option<Action> {
        let (action, key) = self.held?;
        (key == keycode).then(|| {
            self.held = None;
            action
        })
    }

    /// Stops catching `actions`' hotkeys (their grabs were refused).
    fn forget(&mut self, actions: &[Action]) {
        self.keys.retain(|key| !actions.contains(&key.action));
    }
}

/// Grabs the hotkeys' keys while the game is focused, on its own thread.
pub struct KeyGrabber {
    focus: mpsc::Sender<bool>,
}

impl KeyGrabber {
    /// Starts the grabbing thread. A grabbed hotkey sends its Action to
    /// `actions`; the receiver returned yields notes for the player, each
    /// once (keys that can't be grabbed, no X server).
    pub fn spawn(hotkeys: Hotkeys, actions: async_channel::Sender<Action>) -> (KeyGrabber, async_channel::Receiver<String>) {
        let (focus, focus_changes) = mpsc::channel();
        let (notes, notes_rx) = async_channel::unbounded();
        let worker = Worker { hotkeys, actions, notes, told: Vec::new(), x: None };
        std::thread::spawn(move || worker.run(focus_changes));
        (KeyGrabber { focus }, notes_rx)
    }

    /// Grabs the keys while the game is `focused`, releases them otherwise.
    pub fn follow_game(&self, focused: bool) {
        let _ = self.focus.send(focused);
    }
}

/// A connection to the X server, with the plan its key events are read by.
struct X {
    conn: Arc<RustConnection>,
    root: Window,
    plan: Arc<Mutex<GrabPlan>>,
}

struct Worker {
    hotkeys: Hotkeys,
    actions: async_channel::Sender<Action>,
    notes: async_channel::Sender<String>,
    told: Vec<String>,
    x: Option<X>,
}

impl Worker {
    fn run(mut self, focus_changes: mpsc::Receiver<bool>) {
        let mut grabbed = false;
        while let Ok(mut focused) = focus_changes.recv() {
            // Only the latest focus matters.
            while let Ok(next) = focus_changes.try_recv() {
                focused = next;
            }
            if focused != grabbed {
                grabbed = focused && self.grab();
                if !focused {
                    self.release();
                }
            }
        }
    }

    /// Grabs every hotkey's key; false if nothing got grabbed.
    fn grab(&mut self) -> bool {
        let x = match self.x.take().map_or_else(|| self.connect(), Ok) {
            Ok(x) => x,
            Err(err) => {
                self.tell(format!("Hotkeys need the game to run under Xwayland, and no X server answered ({err}). Bind `poe-overlay trigger <action>` in your compositor instead."));
                return false;
            }
        };
        match self.grab_keys(&x) {
            Ok(()) => {
                self.x = Some(x);
                true
            }
            // Xwayland went away: connect anew at the next focus.
            Err(ReplyError::ConnectionError(err)) => {
                eprintln!("poe-overlay: hotkeys: lost the X server: {err}");
                false
            }
            // Some grabs may have gone through: let them go, or they'd eat
            // the keys outside the game.
            Err(err) => {
                eprintln!("poe-overlay: hotkeys not grabbed: {err}");
                self.x = Some(x);
                self.release();
                false
            }
        }
    }

    fn connect(&self) -> Result<X, String> {
        let (conn, screen) = x11rb::connect(None).map_err(|err| err.to_string())?;
        let root = conn.setup().roots[screen].root;
        // Without this a held key repeats as release-press pairs, each
        // running the Action again.
        let xkb = conn.xkb_use_extension(1, 0).ok().and_then(|cookie| cookie.reply().ok());
        if xkb.is_some_and(|reply| reply.supported) {
            let detectable = xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT;
            let none = xkb::BoolCtrl::from(0u32);
            if let Ok(cookie) = conn.xkb_per_client_flags(xkb::ID::USE_CORE_KBD.into(), detectable, detectable, none, none, none) {
                let _ = cookie.reply();
            }
        }
        let conn = Arc::new(conn);
        let plan = Arc::new(Mutex::new(GrabPlan::default()));
        let (reader, plan_read, actions) = (conn.clone(), plan.clone(), self.actions.clone());
        std::thread::spawn(move || read_keys(&reader, &plan_read, &actions));
        Ok(X { conn, root, plan })
    }

    /// Reads the keymap (the layout may have changed since the last grab)
    /// and grabs every hotkey with it; keys that can't be grabbed are told.
    fn grab_keys(&mut self, x: &X) -> Result<(), ReplyError> {
        let mut plan = GrabPlan::new(&self.hotkeys, &keymap(&x.conn)?);
        let mut cookies = Vec::new();
        for grab in plan.grabs() {
            let cookie = x.conn.grab_key(false, x.root, ModMask::from(grab.modifiers), grab.keycode, GrabMode::ASYNC, GrabMode::ASYNC)?;
            cookies.push((grab.action, cookie));
        }
        let mut refused = Vec::new();
        for (action, cookie) in cookies {
            match cookie.check() {
                Ok(()) => {}
                Err(ReplyError::X11Error(err)) if err.error_kind == ErrorKind::Access => {
                    if !refused.contains(&action) {
                        refused.push(action);
                    }
                }
                Err(err) => return Err(err),
            }
        }
        // A hotkey taken in some lock combinations would work only in the
        // others: let it go entirely.
        for grab in plan.grabs().filter(|grab| refused.contains(&grab.action)) {
            x.conn.ungrab_key(grab.keycode, x.root, ModMask::from(grab.modifiers))?;
        }
        plan.forget(&refused);
        for (action, key) in plan.unmapped() {
            self.tell(format!("{key} isn't on your keyboard layout, so {} has no hotkey; {}", action.title(), choose_another()));
        }
        for action in refused {
            let key = self.hotkeys.get(action).map(ToString::to_string).unwrap_or_default();
            self.tell(format!("{key} is taken by another program, so {} has no hotkey; {}", action.title(), choose_another()));
        }
        x.conn.flush()?;
        *x.plan.lock().unwrap_or_else(PoisonError::into_inner) = plan;
        Ok(())
    }

    fn release(&mut self) {
        let Some(x) = &self.x else { return };
        let released = x.conn.ungrab_key(xproto::Grab::ANY, x.root, ModMask::ANY).and_then(|_| x.conn.flush());
        if released.is_err() {
            self.x = None;
        }
    }

    /// Sends `note` to the player unless it was sent before.
    fn tell(&mut self, note: String) {
        if !self.told.contains(&note) {
            eprintln!("poe-overlay: {note}");
            let _ = self.notes.send_blocking(note.clone());
            self.told.push(note);
        }
    }
}

fn choose_another() -> String {
    match crate::config::path() {
        Some(path) => format!("choose another key under [hotkeys] in {}.", path.display()),
        None => "choose another key under [hotkeys] in poe-overlay's config.toml.".to_owned(),
    }
}

fn keymap(conn: &RustConnection) -> Result<Keymap, ReplyError> {
    let setup = conn.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let mapping = conn.get_keyboard_mapping(min, max - min + 1)?.reply()?;
    let modifiers = conn.get_modifier_mapping()?.reply()?;
    Ok(Keymap::new(min, mapping.keysyms_per_keycode, mapping.keysyms, modifiers.keycodes))
}

/// Turns grabbed key events into Actions until the connection closes.
fn read_keys(conn: &RustConnection, plan: &Mutex<GrabPlan>, actions: &async_channel::Sender<Action>) {
    while let Ok(event) = conn.wait_for_event() {
        let mut plan = plan.lock().unwrap_or_else(PoisonError::into_inner);
        let action = match event {
            Event::KeyPress(key) => {
                plan.press(key.detail, key.state.into());
                None
            }
            Event::KeyRelease(key) => plan.release(key.detail),
            _ => None,
        };
        drop(plan);
        if let Some(action) = action
            && actions.send_blocking(action).is_err()
        {
            return;
        }
    }
}
