//! `poe-overlay setup hotkeys [--remove]` from a terminal.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_poe-overlay");

/// Runs the CLI in a session whose home is `home`; `niri` says whether it
/// looks like a niri session.
fn setup(home: &Path, niri: bool, args: &[&str]) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(["setup", "hotkeys"])
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_STATE_HOME", home.join(".local/state"))
        .env_remove("NIRI_CONFIG")
        .env_remove("APPIMAGE");
    if niri {
        cmd.env("NIRI_SOCKET", home.join("niri.sock"));
    } else {
        cmd.env_remove("NIRI_SOCKET");
    }
    cmd.output().unwrap()
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn home_with_niri(config: &str) -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".config/niri")).unwrap();
    fs::write(home.path().join(".config/niri/config.kdl"), config).unwrap();
    home
}

#[test]
fn setup_binds_the_hotkeys_in_niri_and_remove_takes_them_out() {
    let config = "binds {\n    Mod+T { spawn \"alacritty\"; }\n}\n";
    let home = home_with_niri(config);
    let niri = home.path().join(".config/niri");

    let out = setup(home.path(), true, &[]);

    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("Alt+R"), "{}", text(&out));
    let ours = fs::read_to_string(niri.join("poe-overlay.kdl")).unwrap();
    assert!(ours.contains(&format!("spawn \"{BIN}\" \"trigger\" \"currency-rates\"")), "{ours}");

    let out = setup(home.path(), true, &["--remove"]);

    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(fs::read_to_string(niri.join("config.kdl")).unwrap(), config);
    assert!(!niri.join("poe-overlay.kdl").exists());
}

#[test]
fn taken_key_fails_and_says_where_and_what_to_change() {
    let home = home_with_niri("binds {\n    Alt+R { spawn \"firefox\"; }\n}\n");

    let out = setup(home.path(), true, &[]);

    assert!(!out.status.success());
    let text = text(&out);
    assert!(text.contains("config.kdl line 2") && text.contains("[hotkeys]"), "{text}");
}

#[test]
fn other_compositors_get_the_commands_to_bind_by_hand() {
    let home = tempfile::tempdir().unwrap();

    let out = setup(home.path(), false, &[]);

    assert!(!out.status.success());
    let text = text(&out);
    assert!(text.contains("Alt+R") && text.contains("poe-overlay trigger currency-rates"), "{text}");
}

#[test]
fn remove_works_even_with_a_broken_config_toml() {
    let config = "binds {\n}\n";
    let home = home_with_niri(config);
    assert!(setup(home.path(), true, &[]).status.success());
    fs::create_dir_all(home.path().join(".config/poe-overlay")).unwrap();
    fs::write(home.path().join(".config/poe-overlay/config.toml"), "[hotkeys]\ncurrency-rates = \"Hyper+R\"\n").unwrap();

    let out = setup(home.path(), true, &["--remove"]);

    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(fs::read_to_string(home.path().join(".config/niri/config.kdl")).unwrap(), config);
}
