//! Changing settings in config.toml's text while keeping the rest of it.

use std::path::Path;

use poe_overlay::config::{Config, ConfigFile};

const PLAYER_FILE: &str = "\
# My overlay settings
[route]
bandit = \"oak\" # always Oak

[hotkeys]
currency-rates = \"Ctrl+F7\"
";

fn edited(text: &str, edit: impl FnOnce(&mut ConfigFile)) -> String {
    let mut file = ConfigFile::parse(text).unwrap();
    edit(&mut file);
    file.to_string()
}

#[test]
fn empty_file_has_no_league_or_client_txt() {
    let file = ConfigFile::parse("").unwrap();
    assert_eq!(file.league(), None);
    assert_eq!(file.client_txt(), None);
}

#[test]
fn settings_are_read_from_their_sections() {
    let file = ConfigFile::parse("[prices]\nleague = \"Hardcore Covens\"\n[game]\nclient_txt = \"/games/poe/logs/Client.txt\"\n").unwrap();
    assert_eq!(file.league(), Some("Hardcore Covens"));
    assert_eq!(file.client_txt(), Some(Path::new("/games/poe/logs/Client.txt")));
}

#[test]
fn set_settings_are_what_the_overlay_reads() {
    let text = edited(PLAYER_FILE, |file| {
        file.set_league(Some("Standard"));
        file.set_client_txt(Some(Path::new("/games/poe/logs/Client.txt")));
    });
    let config: Config = toml::from_str(&text).unwrap();
    assert_eq!(config.prices.league.as_deref(), Some("Standard"));
    assert_eq!(config.game.client_txt.as_deref(), Some(Path::new("/games/poe/logs/Client.txt")));
}

#[test]
fn setting_keeps_comments_and_other_settings_as_written() {
    let text = edited(PLAYER_FILE, |file| file.set_league(Some("Standard")));
    assert!(text.starts_with(PLAYER_FILE), "{text}");
}

#[test]
fn clearing_a_setting_removes_it_and_its_emptied_section() {
    let text = edited(&format!("{PLAYER_FILE}\n[prices]\nleague = \"Standard\"\n"), |file| file.set_league(None));
    assert_eq!(text.trim_end(), PLAYER_FILE.trim_end());
}

#[test]
fn clearing_keeps_a_section_that_still_has_other_keys() {
    let text = edited("[game]\nclient_txt = \"/a/Client.txt\"\nfuture = 1\n", |file| file.set_client_txt(None));
    assert_eq!(text, "[game]\nfuture = 1\n");
}

#[test]
fn file_with_a_syntax_error_is_refused_with_where() {
    let err = ConfigFile::parse("[prices\nleague = \"Standard\"\n").unwrap_err();
    assert!(err.contains("line 1"), "{err}");
}
