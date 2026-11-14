//! Putting `[hotkeys]` into niri's config: a file of our own, included once
//! from the player's config.kdl.

use std::fs;
use std::path::{Path, PathBuf};

use poe_overlay::hotkeys::niri::Niri;
use poe_overlay::hotkeys::{Fate, Hotkeys};
use poe_overlay::running::Action;

const PROGRAM: &str = "/usr/bin/poe-overlay";

/// A niri config folder: config.kdl including cfg/keybinds.kdl, like the
/// layouts niri users split their config into.
struct Folder {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Folder {
    fn new(keybinds: &str) -> Folder {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("niri");
        fs::create_dir_all(root.join("cfg")).unwrap();
        fs::write(root.join("config.kdl"), "include \"./cfg/keybinds.kdl\"\n\nwindow-rule {\n    open-maximized true\n}\n").unwrap();
        fs::write(root.join("cfg/keybinds.kdl"), keybinds).unwrap();
        Folder { _dir: dir, root }
    }

    fn niri(&self) -> Niri {
        Niri::at(self.root.join("config.kdl"))
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.root.join(name)).unwrap()
    }
}

fn hotkeys(toml: &str) -> Hotkeys {
    toml::from_str::<poe_overlay::config::Config>(toml).unwrap().hotkeys
}

fn setup(niri: &Niri, hotkeys: &Hotkeys) {
    let plan = niri.plan(hotkeys, Path::new(PROGRAM)).unwrap();
    niri.apply(&plan).unwrap();
}

#[test]
fn free_key_is_bound_in_our_own_file_included_once() {
    let folder = Folder::new("binds {\n    Mod+T { spawn \"alacritty\"; }\n}\n");
    let before = folder.read("config.kdl");

    setup(&folder.niri(), &Hotkeys::default());

    let config = folder.read("config.kdl");
    assert!(config.starts_with(&before), "the player's config is only appended to:\n{config}");
    let added = &config[before.len()..];
    assert_eq!(added.matches("include").count(), 1, "{added}");
    assert!(added.contains("include optional=true \"./poe-overlay.kdl\""), "{added}");
    let ours = folder.read("poe-overlay.kdl");
    assert!(ours.contains(&format!("Alt+R hotkey-overlay-title=\"PoE: Currency Rates\" {{ spawn \"{PROGRAM}\" \"trigger\" \"currency-rates\"; }}")), "{ours}");
}

fn fates(niri: &Niri, hotkeys: &Hotkeys) -> Vec<(Action, Fate)> {
    niri.plan(hotkeys, Path::new(PROGRAM)).unwrap().binds.into_iter().map(|planned| (planned.action, planned.fate)).collect()
}

#[test]
fn setting_up_again_changes_nothing() {
    let folder = Folder::new("binds {\n}\n");
    let niri = folder.niri();
    setup(&niri, &Hotkeys::default());
    let (config, ours) = (folder.read("config.kdl"), folder.read("poe-overlay.kdl"));

    let plan = niri.plan(&Hotkeys::default(), Path::new(PROGRAM)).unwrap();
    assert!(plan.installed && plan.up_to_date);
    assert_eq!(plan.binds[0].fate, Fate::Bind, "our own bind is no conflict");
    niri.apply(&plan).unwrap();

    assert_eq!(folder.read("config.kdl"), config);
    assert_eq!(folder.read("poe-overlay.kdl"), ours);
}

#[test]
fn changed_key_makes_the_setup_out_of_date_and_is_rewritten() {
    let folder = Folder::new("binds {\n}\n");
    let niri = folder.niri();
    setup(&niri, &Hotkeys::default());

    let changed = hotkeys("[hotkeys]\ncurrency-rates = \"Ctrl+F7\"");
    let plan = niri.plan(&changed, Path::new(PROGRAM)).unwrap();
    assert!(plan.installed && !plan.up_to_date);
    niri.apply(&plan).unwrap();

    let ours = folder.read("poe-overlay.kdl");
    assert!(ours.contains("Ctrl+F7 ") && !ours.contains("Alt+R"), "{ours}");
    assert_eq!(folder.read("config.kdl").matches("poe-overlay.kdl").count(), 1);
}

#[test]
fn remove_restores_the_config_byte_for_byte() {
    for config in ["include \"./cfg/keybinds.kdl\"\n", "include \"./cfg/keybinds.kdl\"", ""] {
        let folder = Folder::new("binds {\n}\n");
        fs::write(folder.root.join("config.kdl"), config).unwrap();
        let niri = folder.niri();
        setup(&niri, &Hotkeys::default());

        assert!(niri.remove().unwrap());

        assert_eq!(folder.read("config.kdl"), config);
        assert!(!folder.root.join("poe-overlay.kdl").exists());
        assert!(!niri.remove().unwrap(), "nothing left to remove");
    }
}

#[test]
fn key_bound_in_an_included_file_is_taken_and_not_bound_again() {
    let folder = Folder::new("binds {\n    Mod+T { spawn \"alacritty\"; }\n    alt+r { spawn \"firefox\"; }\n}\n");
    let niri = folder.niri();

    let fates = fates(&niri, &Hotkeys::default());

    let location = poe_overlay::hotkeys::Location { file: "cfg/keybinds.kdl".into(), line: 3 };
    assert_eq!(fates, [(Action::CurrencyRates, Fate::Taken(location))]);
    setup(&niri, &Hotkeys::default());
    assert!(!folder.read("poe-overlay.kdl").contains("currency-rates"));
}

#[test]
fn commented_out_binds_do_not_take_a_key() {
    let folder = Folder::new("binds {\n    /-Alt+R { spawn \"firefox\"; }\n    // Alt+R { spawn \"firefox\"; }\n    /* Alt+R { spawn \"firefox\"; } */\n}\n");

    assert_eq!(fates(&folder.niri(), &Hotkeys::default()), [(Action::CurrencyRates, Fate::Bind)]);
}

#[test]
fn action_bound_by_hand_counts_as_done_whatever_its_key() {
    for bind in [
        "Alt+R { spawn \"poe-overlay\" \"trigger\" \"currency-rates\"; }",
        "Super+C { spawn \"/home/me/.cargo/bin/poe-overlay\" \"trigger\" \"currency-rates\"; }",
        "Super+C { spawn-sh \"poe-overlay trigger currency-rates\"; }",
    ] {
        let folder = Folder::new(&format!("binds {{\n    {bind}\n}}\n"));
        let niri = folder.niri();

        let location = poe_overlay::hotkeys::Location { file: "cfg/keybinds.kdl".into(), line: 2 };
        assert_eq!(fates(&niri, &Hotkeys::default()), [(Action::CurrencyRates, Fate::ByHand(location))], "{bind}");
        setup(&niri, &Hotkeys::default());
        assert!(!folder.read("poe-overlay.kdl").contains("currency-rates"), "bound twice: {bind}");
    }
}

#[test]
fn symlinked_config_stays_a_symlink() {
    // Dotfile managers link config.kdl into a repository.
    let folder = Folder::new("binds {\n}\n");
    let real = folder.root.join("../dotfiles-config.kdl");
    fs::rename(folder.root.join("config.kdl"), &real).unwrap();
    std::os::unix::fs::symlink(&real, folder.root.join("config.kdl")).unwrap();

    setup(&folder.niri(), &Hotkeys::default());

    assert!(fs::symlink_metadata(folder.root.join("config.kdl")).unwrap().file_type().is_symlink());
    assert!(fs::read_to_string(&real).unwrap().contains("poe-overlay.kdl"));
    assert!(folder.root.join("poe-overlay.kdl").exists(), "our file sits next to the link, where niri looks");
}

#[test]
fn broken_or_missing_config_is_an_error_and_nothing_is_written() {
    let folder = Folder::new("binds {\n    Alt+R { spawn \"firefox\"\n");
    assert!(folder.niri().plan(&Hotkeys::default(), Path::new(PROGRAM)).unwrap_err().contains("keybinds.kdl"));

    let missing = Niri::at(folder.root.join("nope/config.kdl"));
    assert!(missing.plan(&Hotkeys::default(), Path::new(PROGRAM)).unwrap_err().contains("nope/config.kdl"));
}

#[test]
fn include_moved_into_another_file_is_not_added_again() {
    let folder = Folder::new("binds {\n}\n");
    let niri = folder.niri();
    setup(&niri, &Hotkeys::default());
    let config = folder.read("config.kdl");
    let include = "include optional=true \"../poe-overlay.kdl\"\n";
    fs::write(folder.root.join("config.kdl"), config.replace("include optional=true \"./poe-overlay.kdl\"\n", "")).unwrap();
    fs::write(folder.root.join("cfg/keybinds.kdl"), format!("binds {{\n}}\n{include}")).unwrap();
    let moved = folder.read("config.kdl");

    setup(&niri, &Hotkeys::default());

    assert_eq!(folder.read("config.kdl"), moved);
}

#[test]
fn remove_leaves_an_include_it_did_not_write_and_keeps_the_file_it_needs() {
    // The player dropped `optional=true`: deleting our file would make niri
    // refuse the whole config.
    let folder = Folder::new("binds {\n}\n");
    let niri = folder.niri();
    setup(&niri, &Hotkeys::default());
    let edited = folder.read("config.kdl").replace("include optional=true", "include");
    fs::write(folder.root.join("config.kdl"), &edited).unwrap();

    let err = niri.remove().unwrap_err();

    assert!(err.contains("poe-overlay.kdl"), "{err}");
    assert_eq!(folder.read("config.kdl"), edited);
    assert!(folder.root.join("poe-overlay.kdl").exists());
}
