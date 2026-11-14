//! The bundled campaign Route (exile-leveling) following the Character's Areas.

use poe_overlay::poe1::route::{Bandit, Route, RouteOptions, RouteProgress, Step};

fn texts(steps: &[Step]) -> Vec<&str> {
    steps.iter().map(|s| s.text.as_str()).collect()
}

fn league_start() -> RouteProgress {
    RouteProgress::new(Route::campaign(&RouteOptions::default()))
}

fn with(options: RouteOptions) -> RouteProgress {
    RouteProgress::new(Route::campaign(&options))
}

#[test]
fn route_starts_on_the_twilight_strand() {
    let progress = league_start();

    assert_eq!(progress.area_name(), "The Twilight Strand");
    assert_eq!(texts(progress.steps()), ["Find and kill Hillock", "➞ Lioneye's Watch"]);
}

fn walk(progress: &mut RouteProgress, area_ids: &[&str]) {
    for id in area_ids {
        progress.enter_area(id);
    }
}

#[test]
fn entering_an_area_shows_the_steps_for_it() {
    let mut progress = league_start();

    walk(&mut progress, &["1_1_town", "1_1_2", "1_1_3"]);

    assert_eq!(progress.area_name(), "The Mud Flats");
    assert_eq!(texts(progress.steps()), ["Find 3x Glyph", "➞ The Submerged Passage"]);
    assert_eq!(
        progress.steps()[0].sub_steps,
        ["Little streams connect the 3 Rhoa nests", "Usually form a triangle, but will rarely form a line"]
    );
}

#[test]
fn league_start_route_includes_its_side_trips() {
    let mut progress = league_start();

    walk(&mut progress, &["1_1_town", "1_1_2"]);
    assert_eq!(texts(progress.steps()), ["Get Waypoint", "➞ The Mud Flats"]);

    walk(&mut progress, &["1_1_3", "1_1_4_1"]);
    assert_eq!(texts(progress.steps()), ["Waypoint ➞ The Coast"]);
}

#[test]
fn route_without_league_start_leaves_the_side_trips_out() {
    let mut progress = with(RouteOptions { league_start: false, ..RouteOptions::default() });

    walk(&mut progress, &["1_1_town", "1_1_2"]);
    assert_eq!(texts(progress.steps()), ["➞ The Mud Flats"]);

    walk(&mut progress, &["1_1_3", "1_1_4_1"]);
    assert_eq!(texts(progress.steps()), ["Waypoint ➞ Lioneye's Watch"]);
}

#[test]
fn same_named_area_in_another_act_shows_that_acts_steps() {
    let mut progress = league_start();
    walk(&mut progress, &["1_1_town", "1_1_2"]);

    // Act 6's Lioneye's Watch, not Act 1's (1_1_town).
    progress.enter_area("2_6_town");

    assert_eq!((progress.act(), progress.area_name()), (6, "Lioneye's Watch"));
    assert_eq!(texts(progress.steps()), ["➞ The Twilight Strand"]);
}

#[test]
fn town_trip_in_the_middle_of_an_area_returns_to_its_steps() {
    let mut progress = league_start();
    walk(&mut progress, &["1_1_town", "1_1_2", "1_1_3"]);

    walk(&mut progress, &["1_1_town", "1_1_3"]);

    assert_eq!(texts(progress.steps()), ["Find 3x Glyph", "➞ The Submerged Passage"]);
}

#[test]
fn revisited_area_shows_the_steps_for_that_visit() {
    let mut progress = league_start();
    let tidal_island_trip = ["1_1_town", "1_1_2", "1_1_3", "1_1_4_1", "1_1_2", "1_1_2a"];
    walk(&mut progress, &tidal_island_trip);
    assert_eq!(texts(progress.steps()), ["Find and kill Hailrake, take Medicine Chest", "Logout ➞ Lioneye's Watch"]);
    assert_eq!(
        progress.steps()[0].sub_steps,
        ["Go ←", "If you find a ledge, turn around and go →", "Recommended Level: 4"]
    );

    walk(&mut progress, &["1_1_town", "1_1_4_1"]);
    assert_eq!(texts(progress.steps()), ["➞ The Flooded Depths"]);

    walk(&mut progress, &["1_1_4_0", "1_1_4_1"]);
    assert_eq!(texts(progress.steps()), ["➞ The Ledge"]);
}

#[test]
fn area_outside_the_route_keeps_the_current_steps() {
    let mut progress = league_start();
    walk(&mut progress, &["1_1_town"]);

    progress.enter_area("HideoutLuxurious");

    assert_eq!(texts(progress.steps()), ["Hand in Enemy at the Gate", "➞ The Coast"]);
}

#[test]
fn stepping_back_to_the_previous_area_shows_its_steps_again() {
    // Recorded: The Mud Flats → back to The Coast → The Mud Flats again.
    let mut progress = league_start();
    walk(&mut progress, &["1_1_town", "1_1_2", "1_1_3"]);

    progress.enter_area("1_1_2");
    assert_eq!(texts(progress.steps()), ["Get Waypoint", "➞ The Mud Flats"]);

    progress.enter_area("1_1_3");
    assert_eq!(texts(progress.steps()), ["Find 3x Glyph", "➞ The Submerged Passage"]);
}

#[test]
fn logging_in_again_to_the_same_area_keeps_its_steps() {
    // Recorded: each login logs the town again.
    let mut progress = league_start();
    walk(&mut progress, &["1_1_town"]);

    let changed = progress.enter_area("1_1_town");

    assert!(!changed);
    assert_eq!(texts(progress.steps()), ["Hand in Enemy at the Gate", "➞ The Coast"]);
}

#[test]
fn side_area_off_the_route_is_reported_and_keeps_the_last_steps() {
    let mut progress = league_start();
    walk(&mut progress, &["1_1_town", "1_1_2", "1_1_3"]);

    progress.enter_area("1_1_3a"); // The Fetid Pool, a side area

    assert!(progress.is_off_route());
    assert_eq!(progress.character_area_name(), Some("The Fetid Pool"));
    assert_eq!(texts(progress.steps()), ["Find 3x Glyph", "➞ The Submerged Passage"]);

    progress.enter_area("1_1_3");
    assert!(!progress.is_off_route());
}

#[test]
fn hideout_is_off_the_route_without_a_name() {
    let mut progress = league_start();

    progress.enter_area("HideoutLuxurious");

    assert!(progress.is_off_route());
    assert_eq!(progress.character_area_name(), None);
}

#[test]
fn town_trip_returns_to_the_visit_it_left_even_when_a_later_visit_is_nearer() {
    let mut progress = league_start();
    walk(&mut progress, &["1_1_town", "1_1_2", "1_1_3", "1_1_4_1"]);
    assert_eq!(texts(progress.steps()), ["Waypoint ➞ The Coast"]);

    // Selling in town, then back by waypoint.
    walk(&mut progress, &["1_1_town", "1_1_4_1"]);

    assert_eq!(texts(progress.steps()), ["Waypoint ➞ The Coast"]);
}

#[test]
fn every_act_parses_without_problems_in_every_variant() {
    for league_start in [true, false] {
        for library in [true, false] {
            for bandit in [Bandit::KillAll, Bandit::Oak, Bandit::Kraityn, Bandit::Alira] {
                let options = RouteOptions { league_start, library, bandit };

                let route = Route::campaign(&options);

                assert_eq!(route.problems(), &[] as &[String], "{options:?}");
            }
        }
    }
}

#[test]
fn area_in_a_later_act_shows_that_acts_steps() {
    let mut progress = league_start();

    progress.enter_area("2_6_2");

    assert_eq!((progress.act(), progress.area_name()), (6, "The Coast"));
    assert_eq!(texts(progress.steps()), ["➞ The Mud Flats"]);
}

#[test]
fn portal_placed_in_an_area_is_taken_back_from_town() {
    let mut progress = league_start();

    walk(&mut progress, &["1_2_1", "1_2_town", "1_2_2", "1_2_3", "1_2_town"]);
    assert_eq!(texts(progress.steps()), ["Take Portal ➞ The Old Fields"]);

    progress.enter_area("1_2_2");
    assert_eq!(texts(progress.steps()), ["➞ The Den"]);
}

#[test]
fn bandit_choice_changes_the_steps() {
    let after_the_wetlands = |bandit| {
        let mut progress = with(RouteOptions { bandit, ..RouteOptions::default() });
        walk(&mut progress, &["1_2_12", "1_2_9"]);
        progress.steps()[0].text.clone()
    };

    assert_eq!(after_the_wetlands(Bandit::KillAll), "Kill Alira Darktongue, take Alira's Amulet");
    assert_eq!(after_the_wetlands(Bandit::Alira), "Help Alira Darktongue, take The Apex");
}
