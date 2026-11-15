//! Client.txt lines; all but the chat line were recorded from a real PoE 1
//! session (2026-11-14).

use poe_overlay::poe1::client_log::{LogEvent, check_path, last_character, parse_line};

fn entered(area_id: &str) -> Option<LogEvent> {
    Some(LogEvent::AreaEntered { area_id: area_id.into() })
}

#[test]
fn generating_an_area_means_the_character_entered_it() {
    let line = r#"2026/11/14 14:58:07 12579502 1186a803 [DEBUG Client 312] Generating level 5 area "1_1_4_1" with seed 4172808522"#;

    assert_eq!(parse_line(line), entered("1_1_4_1"));
}

#[test]
fn town_areas_are_entered_too() {
    let line = r#"2026/11/14 17:43:44 22515947 1186a803 [DEBUG Client 308] Generating level 13 area "1_1_town" with seed 1"#;

    assert_eq!(parse_line(line), entered("1_1_town"));
}

#[test]
fn other_lines_are_not_area_changes() {
    let lines = [
        "2026/11/14 14:58:10 12582604 cffb06be [INFO Client 312] : You have entered The Submerged Passage.",
        "2026/11/14 14:58:38 12610278 ca3a6b79 [INFO Client 312] Connecting to instance server at 43.164.114.217:6112 ",
        "2026/11/14 15:03:07 12878884 54ee9dc3 [INFO Client 312] [WINDOW] Lost focus",
        "2026/11/14 14:24:49 ***** LOG FILE OPENING *****",
        "",
    ];

    for line in lines {
        assert_eq!(parse_line(line), None, "{line}");
    }
}

#[test]
fn chat_quoting_the_generating_text_is_not_an_area_change() {
    let line = r#"2026/11/14 15:00:00 12600000 cffb06be [INFO Client 312] #Someone: Generating level 5 area "1_1_4_1" with seed 1"#;

    assert_eq!(parse_line(line), None);
}

#[test]
fn level_up_names_the_character() {
    let line = "2026/11/14 14:55:42 12433885 cffb06be [INFO Client 312] : ExampleExile (Shadow) is now level 3";

    assert_eq!(
        parse_line(line),
        Some(LogEvent::CharacterLevelled { name: "ExampleExile".into() })
    );
}

#[test]
fn deaths_and_chat_mentioning_levels_are_not_level_ups() {
    let lines = [
        "2026/11/14 14:51:36 12188726 cffb06be [INFO Client 312] : ExampleExile has been slain.",
        "2026/11/14 15:00:00 12600000 cffb06be [INFO Client 312] @From buyer: my char (Witch) is now level 90",
        "2026/11/14 15:00:00 12600000 cffb06be [INFO Client 312] #trader: Someone (Witch) is now level 90",
    ];

    for line in lines {
        assert_eq!(parse_line(line), None, "{line}");
    }
}

#[test]
fn last_character_is_the_latest_level_up_in_the_history() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Client.txt");
    std::fs::write(
        &log,
        "2026/11/14 14:24:49 ***** LOG FILE OPENING *****\r\n\
         2026/11/14 14:32:28 11040103 cffb06be [INFO Client 316] : oldchar (Witch) is now level 40\r\n\
         2026/11/14 14:55:42 12433885 cffb06be [INFO Client 312] : ExampleExile (Shadow) is now level 3\r\n\
         2026/11/14 14:58:07 12579502 1186a803 [DEBUG Client 312] Generating level 5 area \"1_1_4_1\" with seed 4172808522\r\n",
    )
    .unwrap();

    assert_eq!(last_character(&log).as_deref(), Some("ExampleExile"));
}

#[test]
fn no_last_character_without_level_ups_or_log() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Client.txt");
    assert_eq!(last_character(&log), None);

    std::fs::write(&log, "2026/11/14 14:24:49 ***** LOG FILE OPENING *****\r\n").unwrap();
    assert_eq!(last_character(&log), None);
}

#[test]
fn last_character_is_found_far_back_in_a_long_log() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Client.txt");
    let mut text = String::from("2026/11/14 14:55:42 12433885 cffb06be [INFO Client 312] : ExampleExile (Shadow) is now level 3\r\n");
    for i in 0..20_000 {
        text += &format!("2026/11/14 15:03:07 {i} 54ee9dc3 [INFO Client 312] [WINDOW] Lost focus\r\n");
    }
    std::fs::write(&log, text).unwrap();

    assert_eq!(last_character(&log).as_deref(), Some("ExampleExile"));
}

#[test]
fn level_up_line_split_across_read_chunks_is_still_found() {
    // Places the level-up line across the 64 KiB boundary counted from the end.
    let level_up = "2026/11/14 14:55:42 12433885 cffb06be [INFO Client 312] : ExampleExile (Shadow) is now level 3\r\n";
    let filler_line = "2026/11/14 15:03:07 12878884 54ee9dc3 [INFO Client 312] [WINDOW] Lost focus\r\n";
    let mut tail = String::new();
    while tail.len() + level_up.len() / 2 < 64 * 1024 {
        tail += filler_line;
    }
    let tail = &tail[tail.len() - (64 * 1024 - level_up.len() / 2)..];
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Client.txt");
    std::fs::write(&log, format!("{filler_line}{level_up}{tail}")).unwrap();

    assert_eq!(last_character(&log).as_deref(), Some("ExampleExile"));
}

#[test]
fn chosen_client_txt_that_exists_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Client.txt");
    std::fs::write(&log, "").unwrap();

    assert_eq!(check_path(&log), Ok(()));
}

#[test]
fn chosen_client_txt_problems_are_named() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let unreadable = dir.path().join("locked.txt");
    std::fs::write(&unreadable, "").unwrap();
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();

    let mut cases = vec![(dir.path().join("missing/Client.txt"), "No file at"), (dir.path().to_owned(), "is a folder")];
    // Root reads any file.
    if !rustix::process::geteuid().is_root() {
        cases.push((unreadable, "Can't read"));
    }
    for (path, expected) in cases {
        let err = check_path(&path).unwrap_err();
        assert!(err.contains(expected) && err.contains(&*path.to_string_lossy()), "{path:?}: {err}");
    }
}

#[test]
fn chosen_client_txt_must_be_a_full_path() {
    let err = check_path(std::path::Path::new("logs/Client.txt")).unwrap_err();
    assert!(err.contains("full path"), "{err}");
}
