//! Following Client.txt as the game appends to it.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use poe_overlay::log_follow::LogFollower;

fn append(path: &Path, text: &str) {
    OpenOptions::new().create(true).append(true).open(path).unwrap().write_all(text.as_bytes()).unwrap();
}

#[test]
fn history_written_before_start_is_not_replayed() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Client.txt");
    append(&log, "old line 1\r\nold line 2\r\n");

    let mut follower = LogFollower::new(&log);

    assert!(follower.poll().unwrap().is_empty());
}

#[test]
fn lines_appended_after_start_are_read_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Client.txt");
    append(&log, "old\r\n");
    let mut follower = LogFollower::new(&log);

    append(&log, "first\r\nsecond\rthird\n");

    assert_eq!(follower.poll().unwrap(), ["first", "second", "third"]);
    assert!(follower.poll().unwrap().is_empty());
}

#[test]
fn half_written_line_is_read_once_complete() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Client.txt");
    append(&log, "");
    let mut follower = LogFollower::new(&log);

    append(&log, "Generating level 2 area \"1_1");
    assert!(follower.poll().unwrap().is_empty());
    append(&log, "_2\" with seed 1\r\n");

    assert_eq!(follower.poll().unwrap(), ["Generating level 2 area \"1_1_2\" with seed 1"]);
}

#[test]
fn log_created_after_start_is_read_from_its_beginning() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Client.txt");
    let mut follower = LogFollower::new(&log);
    assert!(follower.poll().unwrap().is_empty());

    append(&log, "first\r\n");

    assert_eq!(follower.poll().unwrap(), ["first"]);
}

#[test]
fn truncated_log_is_read_from_its_new_beginning() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Client.txt");
    append(&log, "a long history line\r\n");
    let mut follower = LogFollower::new(&log);

    fs::write(&log, "new\r\n").unwrap();

    assert_eq!(follower.poll().unwrap(), ["new"]);
}
