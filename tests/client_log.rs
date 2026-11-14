//! Client.txt lines recorded from a real PoE 1 session (2026-11-14).

use poe_overlay::poe1::client_log::{LogEvent, parse_line};

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
