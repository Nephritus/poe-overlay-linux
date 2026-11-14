//! Hotkeys chosen in config.toml's `[hotkeys]`.

use std::fs;
use std::path::PathBuf;

use poe_overlay::config::Config;
use poe_overlay::hotkeys::niri::Niri;
use poe_overlay::hotkeys::{Answers, Compositor, Fate, HotkeySetup, Hotkeys, LaunchPrompt};
use poe_overlay::running::Action;

fn config(text: &str) -> Result<Config, String> {
    toml::from_str(text).map_err(|err| err.to_string())
}

#[test]
fn currency_rates_is_on_alt_r_unless_configured() {
    let hotkeys = config("").unwrap().hotkeys;

    assert_eq!(hotkeys.get(Action::CurrencyRates).map(ToString::to_string).as_deref(), Some("Alt+R"));
}

#[test]
fn empty_key_leaves_the_action_without_a_hotkey() {
    let hotkeys = config("[hotkeys]\ncurrency-rates = \"\"").unwrap().hotkeys;

    assert!(hotkeys.get(Action::CurrencyRates).is_none());
    assert_eq!(hotkeys.iter().count(), 0);
}

#[test]
fn configured_key_replaces_the_default() {
    let hotkeys = config("[hotkeys]\ncurrency-rates = \"ctrl+shift+F5\"").unwrap().hotkeys;

    assert_eq!(hotkeys.get(Action::CurrencyRates).unwrap().to_string(), "Ctrl+Shift+F5");
}

#[test]
fn mistakes_in_hotkeys_are_reported_with_what_is_wrong() {
    for (value, expected) in [
        ("[hotkeys]\ncurrency-rates = \"Alt+Rr\"", "unknown key `Rr`"),
        ("[hotkeys]\ncurrency-rates = \"Hyper+R\"", "unknown modifier `Hyper`"),
        ("[hotkeys]\ncurrency-rates = \"Alt+\"", "no key"),
        // `Mod` means different keys in different compositors.
        ("[hotkeys]\ncurrency-rates = \"Mod+R\"", "unknown modifier `Mod`"),
        ("[hotkeys]\ncurrency = \"Alt+R\"", "currency-rates"),
        ("[hotkeys]\ncurrency-rates = 5", "must be a string"),
    ] {
        let err = config(value).unwrap_err();
        assert!(err.contains(expected), "{value:?}: {err}");
    }
}

/// A niri config folder and a state folder, kept across "restarts" (new
/// HotkeySetup values).
struct Session {
    dir: tempfile::TempDir,
}

impl Session {
    fn new() -> Session {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("niri")).unwrap();
        fs::write(dir.path().join("niri/config.kdl"), "binds {\n    Mod+T { spawn \"alacritty\"; }\n}\n").unwrap();
        Session { dir }
    }

    fn niri_file(&self, name: &str) -> PathBuf {
        self.dir.path().join("niri").join(name)
    }

    /// The overlay as it starts with the game, with `hotkeys` from config.toml.
    fn start(&self, hotkeys: Hotkeys) -> HotkeySetup {
        let compositor = Compositor::Niri(Niri::at(self.niri_file("config.kdl")));
        self.start_on(compositor, hotkeys)
    }

    fn start_on(&self, compositor: Compositor, hotkeys: Hotkeys) -> HotkeySetup {
        let answers = Answers::at(self.dir.path().join("state/hotkeys.toml"));
        HotkeySetup::new(compositor, hotkeys, PathBuf::from("/usr/bin/poe-overlay"), answers)
    }
}

fn asks_to_bind(prompt: &LaunchPrompt) -> bool {
    matches!(prompt, LaunchPrompt::Ask(plan) if plan.to_bind().next().is_some())
}

#[test]
fn first_launch_asks_and_set_up_stops_the_asking() {
    let session = Session::new();

    let overlay = session.start(Hotkeys::default());
    assert!(asks_to_bind(&overlay.at_launch()));
    overlay.set_up().unwrap();

    assert!(matches!(session.start(Hotkeys::default()).at_launch(), LaunchPrompt::Nothing));
    assert!(fs::read_to_string(session.niri_file("poe-overlay.kdl")).unwrap().contains("currency-rates"));
}

#[test]
fn not_now_is_remembered_across_restarts() {
    let session = Session::new();
    session.start(Hotkeys::default()).decline().unwrap();

    assert!(matches!(session.start(Hotkeys::default()).at_launch(), LaunchPrompt::Nothing));
    assert!(!session.niri_file("poe-overlay.kdl").exists());
}

#[test]
fn key_changed_after_set_up_is_applied_at_launch_without_asking() {
    let session = Session::new();
    session.start(Hotkeys::default()).set_up().unwrap();

    let changed = config("[hotkeys]\ncurrency-rates = \"Ctrl+F7\"").unwrap().hotkeys;
    assert!(matches!(session.start(changed).at_launch(), LaunchPrompt::Nothing));

    let ours = fs::read_to_string(session.niri_file("poe-overlay.kdl")).unwrap();
    assert!(ours.contains("Ctrl+F7 ") && !ours.contains("Alt+R"), "{ours}");
}

#[test]
fn key_taken_after_set_up_is_reported_and_left_to_the_player() {
    let session = Session::new();
    session.start(Hotkeys::default()).set_up().unwrap();
    let config_kdl = session.niri_file("config.kdl");
    let text = fs::read_to_string(&config_kdl).unwrap().replacen("binds {\n", "binds {\n    Alt+R { spawn \"firefox\"; }\n", 1);
    fs::write(&config_kdl, text).unwrap();

    let prompt = session.start(Hotkeys::default()).at_launch();

    assert!(matches!(&prompt, LaunchPrompt::Conflicts(plan) if matches!(plan.binds[0].fate, Fate::Taken(_))), "{prompt:?}");
    assert!(!fs::read_to_string(session.niri_file("poe-overlay.kdl")).unwrap().contains("Alt+R"), "ours would override the player's bind");
}

#[test]
fn action_bound_by_hand_is_not_asked_about() {
    let session = Session::new();
    fs::write(session.niri_file("config.kdl"), "binds {\n    Alt+R { spawn \"poe-overlay\" \"trigger\" \"currency-rates\"; }\n}\n").unwrap();

    assert!(matches!(session.start(Hotkeys::default()).at_launch(), LaunchPrompt::Nothing));
}

#[test]
fn unknown_compositor_shows_the_commands_until_dismissed() {
    let session = Session::new();
    let overlay = session.start_on(Compositor::Unknown, Hotkeys::default());
    assert!(matches!(overlay.at_launch(), LaunchPrompt::Manual));

    overlay.decline().unwrap();

    assert!(matches!(session.start_on(Compositor::Unknown, Hotkeys::default()).at_launch(), LaunchPrompt::Nothing));
}

#[test]
fn remove_takes_the_binds_out_and_stops_the_asking() {
    let session = Session::new();
    let before = fs::read_to_string(session.niri_file("config.kdl")).unwrap();
    session.start(Hotkeys::default()).set_up().unwrap();

    assert!(session.start(Hotkeys::default()).remove().unwrap());

    assert_eq!(fs::read_to_string(session.niri_file("config.kdl")).unwrap(), before);
    assert!(matches!(session.start(Hotkeys::default()).at_launch(), LaunchPrompt::Nothing));
}

#[test]
fn remove_elsewhere_stops_the_commands_from_being_shown() {
    let session = Session::new();

    assert!(!session.start_on(Compositor::Unknown, Hotkeys::default()).remove().unwrap());

    assert!(matches!(session.start_on(Compositor::Unknown, Hotkeys::default()).at_launch(), LaunchPrompt::Nothing));
}

#[test]
fn set_up_after_not_now_asks_again_if_the_binds_go_missing() {
    let session = Session::new();
    let before = fs::read_to_string(session.niri_file("config.kdl")).unwrap();
    session.start(Hotkeys::default()).decline().unwrap();
    session.start(Hotkeys::default()).set_up().unwrap();

    // The player restores their config from a backup.
    fs::write(session.niri_file("config.kdl"), before).unwrap();

    assert!(asks_to_bind(&session.start(Hotkeys::default()).at_launch()));
}

#[test]
fn dismissing_the_commands_elsewhere_does_not_skip_the_niri_question() {
    // e.g. a launch from a session without NIRI_SOCKET, then a normal one.
    let session = Session::new();
    session.start_on(Compositor::Unknown, Hotkeys::default()).decline().unwrap();

    assert!(asks_to_bind(&session.start(Hotkeys::default()).at_launch()));
}
