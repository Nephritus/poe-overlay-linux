//! Finding PoE 1's Client.txt through Steam's library folders.

use std::fs;
use std::path::{Path, PathBuf};

use poe_overlay::poe1::client_log_path;

/// A Steam root whose `libraryfolders.vdf` lists `libraries` as
/// `(path, app ids)`, laid out as Steam writes it.
fn steam_root(root: &Path, libraries: &[(&Path, &[&str])]) {
    let mut vdf = String::from("\"libraryfolders\"\n{\n");
    for (i, (path, apps)) in libraries.iter().enumerate() {
        vdf += &format!("\t\"{i}\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"label\"\t\t\"\"\n\t\t\"apps\"\n\t\t{{\n", path.display());
        for app in *apps {
            vdf += &format!("\t\t\t\"{app}\"\t\t\"74730763743\"\n");
        }
        vdf += "\t\t}\n\t}\n";
    }
    vdf += "}\n";
    fs::create_dir_all(root.join("steamapps")).unwrap();
    fs::write(root.join("steamapps/libraryfolders.vdf"), vdf).unwrap();
}

fn install_poe(library: &Path, install_dir: &str) {
    fs::create_dir_all(library.join("steamapps")).unwrap();
    fs::write(
        library.join("steamapps/appmanifest_238960.acf"),
        format!("\"AppState\"\n{{\n\t\"appid\"\t\t\"238960\"\n\t\"name\"\t\t\"Path of Exile\"\n\t\"installdir\"\t\t\"{install_dir}\"\n}}\n"),
    )
    .unwrap();
}

#[test]
fn client_log_is_found_in_the_main_steam_library() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".local/share/Steam");
    steam_root(&root, &[(&root, &["730", "238960"])]);
    install_poe(&root, "Path of Exile");

    let found = client_log_path(std::slice::from_ref(&root));

    assert_eq!(found, Some(root.join("steamapps/common/Path of Exile/logs/Client.txt")));
}

#[test]
fn client_log_is_found_in_another_steam_library() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".local/share/Steam");
    let games = home.path().join("games/SteamLibrary");
    steam_root(&root, &[(&root, &["730"]), (&games, &["238960"])]);
    install_poe(&games, "PoE");

    let found = client_log_path(&[PathBuf::from("/nonexistent/steam"), root]);

    assert_eq!(found, Some(games.join("steamapps/common/PoE/logs/Client.txt")));
}

#[test]
fn no_client_log_when_the_game_is_not_installed() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".local/share/Steam");
    steam_root(&root, &[(&root, &["730"])]);

    assert_eq!(client_log_path(&[root]), None);
}
