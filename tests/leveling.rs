//! A leveling session: Client.txt events and Previous/Next driving Route
//! progress that is saved per Character and restored after a restart.

use std::path::{Path, PathBuf};

use poe_overlay::poe1::client_log::LogEvent;
use poe_overlay::poe1::leveling::{Leveling, ProgressStore};
use poe_overlay::poe1::route::{Route, RouteOptions};

fn store_path(dir: &Path) -> PathBuf {
    dir.join("progress.toml")
}

/// Starts the overlay: a fresh Route, the saved progress, and the Character
/// guessed from Client.txt history.
fn start(dir: &Path, assumed: Option<&str>) -> Leveling {
    let (store, warning) = ProgressStore::open(store_path(dir));
    assert_eq!(warning, None);
    Leveling::new(Route::campaign(&RouteOptions::default()), store, assumed.map(String::from))
}

fn enter(leveling: &mut Leveling, area_ids: &[&str]) {
    for id in area_ids {
        leveling.handle(&LogEvent::AreaEntered { area_id: (*id).into() });
    }
}

fn level_up(leveling: &mut Leveling, name: &str) {
    leveling.handle(&LogEvent::CharacterLevelled { name: name.into(), class: "Shadow".into(), level: 5 });
}

fn area(leveling: &Leveling) -> &str {
    leveling.progress().area_name()
}

#[test]
fn restart_restores_the_last_area_visit_of_the_character() {
    let dir = tempfile::tempdir().unwrap();
    let mut first = start(dir.path(), None);
    level_up(&mut first, "jay");
    enter(&mut first, &["1_1_town", "1_1_2", "1_1_3"]);
    drop(first);

    let restarted = start(dir.path(), Some("jay"));

    assert_eq!(restarted.character(), Some("jay"));
    assert_eq!(area(&restarted), "The Mud Flats");
}

#[test]
fn progress_before_the_character_is_known_attaches_once_it_levels_up() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = start(dir.path(), None);
    enter(&mut session, &["1_1_town", "1_1_2", "1_1_3"]);
    assert_eq!(session.character(), None);

    level_up(&mut session, "jay");
    drop(session);

    assert_eq!(area(&start(dir.path(), Some("jay"))), "The Mud Flats");
}

#[test]
fn route_follows_areas_before_the_character_is_known() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = start(dir.path(), None);

    enter(&mut session, &["1_1_town", "1_1_2"]);

    assert_eq!(area(&session), "The Coast");
}

#[test]
fn two_characters_keep_separate_progress() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = start(dir.path(), None);
    enter(&mut session, &["1_1_town", "1_1_2", "1_1_3"]);
    level_up(&mut session, "main");

    // An alt logs in at its own town and levels up again at The Coast.
    enter(&mut session, &["1_1_town"]);
    level_up(&mut session, "alt");
    enter(&mut session, &["1_1_2"]);
    level_up(&mut session, "alt");

    // Back on the main, which logs in where it left off.
    enter(&mut session, &["1_1_3"]);
    level_up(&mut session, "main");
    assert_eq!(area(&session), "The Mud Flats");
    drop(session);

    assert_eq!(area(&start(dir.path(), Some("main"))), "The Mud Flats");
    assert_eq!(area(&start(dir.path(), Some("alt"))), "The Coast");
}

#[test]
fn wrong_guess_at_startup_does_not_overwrite_the_guessed_characters_progress() {
    let dir = tempfile::tempdir().unwrap();
    let mut setup = start(dir.path(), None);
    level_up(&mut setup, "main");
    enter(&mut setup, &["1_1_town", "1_1_2", "1_1_3"]);
    drop(setup);

    // History says "main", but the player logs in on another Character.
    let mut session = start(dir.path(), Some("main"));
    enter(&mut session, &["1_1_town", "1_1_2"]);
    level_up(&mut session, "alt");
    assert_eq!(area(&session), "The Coast");
    drop(session);

    assert_eq!(area(&start(dir.path(), Some("main"))), "The Mud Flats");
    assert_eq!(area(&start(dir.path(), Some("alt"))), "The Coast");
}

#[test]
fn returning_character_resumes_its_saved_visit_of_the_area_it_is_in() {
    let dir = tempfile::tempdir().unwrap();
    let mut setup = start(dir.path(), None);
    level_up(&mut setup, "main");
    // Second Submerged Passage visit, after the Tidal Island trip.
    enter(&mut setup, &["1_1_town", "1_1_2", "1_1_3", "1_1_4_1", "1_1_2", "1_1_2a", "1_1_town", "1_1_4_1"]);
    drop(setup);

    // History guesses an alt; main logs in at the Passage and levels up.
    let mut session = start(dir.path(), Some("alt"));
    enter(&mut session, &["1_1_4_1"]);
    level_up(&mut session, "main");

    assert_eq!(session.progress().steps()[0].text, "➞ The Flooded Depths");
}

#[test]
fn switching_characters_keeps_each_ones_position_at_its_last_level_up() {
    // Client.txt doesn't say when a different Character logs in, so Areas
    // entered after a Character's last level-up can't be told apart.
    let dir = tempfile::tempdir().unwrap();
    let mut session = start(dir.path(), None);
    level_up(&mut session, "main");
    enter(&mut session, &["1_1_town", "1_1_2"]);
    level_up(&mut session, "main");
    enter(&mut session, &["1_1_3"]);

    enter(&mut session, &["1_1_town"]);
    level_up(&mut session, "alt");
    drop(session);

    assert_eq!(area(&start(dir.path(), Some("main"))), "The Coast");
}

#[test]
fn returning_character_continues_from_the_area_it_logged_in_to() {
    let dir = tempfile::tempdir().unwrap();
    let mut setup = start(dir.path(), None);
    enter(&mut setup, &["1_1_town", "1_1_2"]);
    level_up(&mut setup, "main");
    drop(setup);

    // Played on to The Mud Flats in a session that saved it under a wrong guess.
    let mut session = start(dir.path(), Some("alt"));
    enter(&mut session, &["1_1_3"]);
    level_up(&mut session, "main");

    assert_eq!(area(&session), "The Mud Flats");
}

#[test]
fn previous_and_next_corrections_are_saved() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = start(dir.path(), None);
    level_up(&mut session, "jay");

    assert!(session.next_leg());
    assert!(session.next_leg());
    assert!(session.previous_leg());
    drop(session);

    assert_eq!(area(&start(dir.path(), Some("jay"))), "Lioneye's Watch");
}

#[test]
fn unreadable_progress_file_is_kept_aside_and_progress_starts_fresh() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(store_path(dir.path()), "not = [valid").unwrap();

    let (store, warning) = ProgressStore::open(store_path(dir.path()));

    assert!(warning.is_some());
    assert!(dir.path().join("progress.toml.corrupt").exists());
    let session = Leveling::new(Route::campaign(&RouteOptions::default()), store, Some("jay".into()));
    assert_eq!(area(&session), "The Twilight Strand");
}
