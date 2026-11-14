//! Hotkeys grabbed through Xwayland: which keys get grabbed, from a keymap
//! dumped from a real Xwayland (US layout, NumLock on Mod2).

use std::collections::BTreeSet;

use poe_overlay::hotkeys::Hotkeys;
use poe_overlay::key_grab::{Grab, GrabPlan, Keymap};
use poe_overlay::running::Action;

// X modifier mask bits.
const SHIFT: u16 = 1;
const LOCK: u16 = 1 << 1;
const CONTROL: u16 = 1 << 2;
const MOD1: u16 = 1 << 3;
const MOD2: u16 = 1 << 4;
const MOD4: u16 = 1 << 6;
const MOD5: u16 = 1 << 7;

// Keycodes in the dumped keymap.
const R: u8 = 27;
const D: u8 = 40;
const F5: u8 = 71;
const ALT_L: u8 = 64;

fn keymap() -> Keymap {
    keymap_with(|_| {})
}

/// The dumped keymap, changed by `edit`.
struct Fixture {
    keysyms_per_keycode: usize,
    keysyms: Vec<u32>,
    /// The modifier map: 8 rows of keycodes, Shift to Mod5.
    rows: Vec<Vec<u8>>,
}

impl Fixture {
    /// Makes `keycode` type `keysym` in `column` (0: first layout, 2: second).
    fn set(&mut self, keycode: u8, column: usize, keysym: u32) {
        self.keysyms[usize::from(keycode - MIN_KEYCODE) * self.keysyms_per_keycode + column] = keysym;
    }
}

const MIN_KEYCODE: u8 = 8;

fn keymap_with(edit: impl FnOnce(&mut Fixture)) -> Keymap {
    let json: serde_json::Value = serde_json::from_str(include_str!("data/x-keymap-us.json")).unwrap();
    assert_eq!(json["min_keycode"], MIN_KEYCODE);
    let numbers = |key: &str| json[key].as_array().unwrap().iter().map(|n| n.as_u64().unwrap()).collect::<Vec<_>>();
    let per_modifier = json["keycodes_per_modifier"].as_u64().unwrap() as usize;
    let mut fixture = Fixture {
        keysyms_per_keycode: json["keysyms_per_keycode"].as_u64().unwrap() as usize,
        keysyms: numbers("keysyms").into_iter().map(|k| k as u32).collect(),
        rows: numbers("modifier_keycodes").chunks(per_modifier).map(|row| row.iter().map(|&k| k as u8).collect()).collect(),
    };
    edit(&mut fixture);
    Keymap::new(MIN_KEYCODE, fixture.keysyms_per_keycode as u8, fixture.keysyms, fixture.rows.concat())
}

fn hotkeys(text: &str) -> Hotkeys {
    toml::from_str(text).unwrap()
}

fn grabs(plan: &GrabPlan) -> BTreeSet<(u8, u16)> {
    plan.grabs().map(|Grab { keycode, modifiers, .. }| (keycode, modifiers)).collect()
}

/// The grabs for `keycode` with `modifiers`, whatever the lock keys are.
fn with_locks(keycode: u8, modifiers: u16) -> BTreeSet<(u8, u16)> {
    [0, LOCK, MOD2, LOCK | MOD2].into_iter().map(|locks| (keycode, modifiers | locks)).collect()
}

#[test]
fn alt_r_is_grabbed_on_the_r_key_with_mod1_whatever_the_lock_keys() {
    let plan = GrabPlan::new(&hotkeys("currency-rates = \"Alt+R\"\nprice-check = \"\""), &keymap());
    assert_eq!(grabs(&plan), with_locks(R, MOD1));
}

#[test]
fn ctrl_and_shift_are_the_core_modifiers() {
    let plan = GrabPlan::new(&hotkeys("currency-rates = \"Ctrl+Shift+F5\"\nprice-check = \"\""), &keymap());
    assert_eq!(grabs(&plan), with_locks(F5, CONTROL | SHIFT));
}

#[test]
fn alt_and_super_are_found_through_the_modifier_map() {
    // A layout with Alt keys on Mod4 and Super keys on Mod1.
    let plan = GrabPlan::new(&hotkeys("currency-rates = \"Alt+R\"\nprice-check = \"Super+D\""), &keymap_with(|keymap| keymap.rows.swap(3, 6)));
    let mut expected = with_locks(R, MOD4);
    expected.extend(with_locks(D, MOD1));
    assert_eq!(grabs(&plan), expected);
}

#[test]
fn every_action_with_a_hotkey_is_grabbed() {
    let plan = GrabPlan::new(&Hotkeys::default(), &keymap());
    let mut expected = with_locks(R, MOD1);
    expected.extend(with_locks(D, MOD1));
    assert_eq!(grabs(&plan), expected);
}

#[test]
fn key_missing_from_the_layout_is_reported_and_not_grabbed() {
    let plan = GrabPlan::new(&hotkeys("currency-rates = \"Alt+F35\""), &keymap());
    let missing: Vec<_> = plan.unmapped().map(|(action, key)| (action, key.to_string())).collect();
    assert_eq!(missing, [(Action::CurrencyRates, "Alt+F35".to_owned())]);
    assert_eq!(grabs(&plan), with_locks(D, MOD1));
}

#[test]
fn key_is_grabbed_where_the_first_layout_has_it() {
    // `us,de`: the second layout swaps Y and Z; Alt+Z stays on the us Z key.
    let (y, z) = (29, 52);
    let plan = GrabPlan::new(
        &hotkeys("currency-rates = \"Alt+Z\"\nprice-check = \"\""),
        &keymap_with(|keymap| {
            keymap.set(y, 2, 'z' as u32);
            keymap.set(z, 2, 'y' as u32);
        }),
    );
    assert_eq!(grabs(&plan), with_locks(z, MOD1));
}

#[test]
fn key_only_in_the_second_layout_is_grabbed_there() {
    // `ru,us`: the R key types the Cyrillic er first, r in the second layout.
    const CYRILLIC_ER: u32 = 0x6d2;
    let plan = GrabPlan::new(&hotkeys("currency-rates = \"Alt+R\"\nprice-check = \"\""), &keymap_with(|keymap| keymap.set(R, 0, CYRILLIC_ER)));
    assert_eq!(grabs(&plan), with_locks(R, MOD1));
}

#[test]
fn key_typed_only_with_shift_is_reported_not_grabbed_unshifted() {
    let plan = GrabPlan::new(&hotkeys("currency-rates = \"Alt+colon\"\nprice-check = \"\""), &keymap());
    assert_eq!(plan.unmapped().count(), 1);
    assert!(grabs(&plan).is_empty());
}

#[test]
fn hotkey_runs_its_action_when_its_key_is_released() {
    let mut plan = GrabPlan::new(&Hotkeys::default(), &keymap());
    plan.press(D, MOD1);
    assert_eq!(plan.release(D), Some(Action::PriceCheck));
    plan.press(R, MOD1);
    assert_eq!(plan.release(R), Some(Action::CurrencyRates));
}

#[test]
fn held_hotkey_runs_its_action_once() {
    let mut plan = GrabPlan::new(&Hotkeys::default(), &keymap());
    // Auto-repeat sends more presses and one release.
    for _ in 0..3 {
        plan.press(D, MOD1);
    }
    assert_eq!(plan.release(D), Some(Action::PriceCheck));
    assert_eq!(plan.release(D), None);
}

#[test]
fn releasing_alt_before_the_key_still_runs_the_action() {
    let mut plan = GrabPlan::new(&Hotkeys::default(), &keymap());
    plan.press(D, MOD1);
    assert_eq!(plan.release(ALT_L), None);
    assert_eq!(plan.release(D), Some(Action::PriceCheck));
}

#[test]
fn lock_keys_do_not_stop_a_hotkey() {
    let mut plan = GrabPlan::new(&Hotkeys::default(), &keymap());
    plan.press(D, MOD1 | LOCK | MOD2);
    assert_eq!(plan.release(D), Some(Action::PriceCheck));
}

#[test]
fn other_modifiers_or_keys_run_nothing() {
    let mut plan = GrabPlan::new(&Hotkeys::default(), &keymap());
    for (keycode, state) in [(D, MOD1 | SHIFT), (D, 0), (F5, MOD1)] {
        plan.press(keycode, state);
        assert_eq!(plan.release(keycode), None, "keycode {keycode} with state {state:#x}");
    }
}

#[test]
fn num_lock_is_found_through_the_modifier_map() {
    // A layout with NumLock on Mod5 (and the ISO level-3 shift on Mod2).
    let plan = GrabPlan::new(&hotkeys("currency-rates = \"Alt+R\"\nprice-check = \"\""), &keymap_with(|keymap| keymap.rows.swap(4, 7)));
    let expected: BTreeSet<_> = [0, LOCK, MOD5, LOCK | MOD5].into_iter().map(|locks| (R, MOD1 | locks)).collect();
    assert_eq!(grabs(&plan), expected);
}
