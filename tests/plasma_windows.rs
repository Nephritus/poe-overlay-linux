//! KDE Plasma's window list (org_kde_plasma_window_management) turned into
//! the toplevel updates that decide where the Overlay shows.

use poe_overlay::focus::{GameFocus, Rect, Toplevel, ToplevelEvent, Visibility};
use poe_overlay::plasma_windows::{PlasmaEvent, PlasmaWindows};

const ACTIVE: u32 = 0x1;
const FULLSCREEN: u32 = 0x8;

const OUTPUTS: [(&str, Rect); 2] = [
    ("DP-1", Rect { x: 0, y: 0, width: 1920, height: 1080 }),
    ("HDMI-A-1", Rect { x: 1920, y: 0, width: 2560, height: 1440 }),
];

const ON_HDMI: Rect = Rect { x: 1920, y: 0, width: 2560, height: 1440 };

/// What KWin sends for a new window, up to and including its initial state.
fn announce(windows: &mut PlasmaWindows, id: u32, app_id: &str, title: &str, state: u32) -> Vec<ToplevelEvent> {
    [
        PlasmaEvent::AppId(app_id.into()),
        PlasmaEvent::Title(title.into()),
        PlasmaEvent::State(state),
        PlasmaEvent::Geometry(ON_HDMI),
        PlasmaEvent::InitialState,
    ]
    .into_iter()
    .filter_map(|event| windows.apply(id, event, OUTPUTS))
    .collect()
}

#[test]
fn a_window_is_reported_once_its_initial_state_is_complete() {
    let mut windows = PlasmaWindows::default();

    let reported = announce(&mut windows, 7, "steam_app_238960", "Path of Exile", FULLSCREEN | ACTIVE);

    let game = Toplevel { app_id: "steam_app_238960".into(), title: "Path of Exile".into(), activated: true, outputs: vec!["HDMI-A-1".into()] };
    assert_eq!(reported, [ToplevelEvent::Updated { id: 7, toplevel: game }]);
}

#[test]
fn later_changes_are_reported_one_by_one() {
    let mut windows = PlasmaWindows::default();
    announce(&mut windows, 7, "steam_app_238960", "Path of Exile", FULLSCREEN | ACTIVE);

    let deactivated = windows.apply(7, PlasmaEvent::State(FULLSCREEN), OUTPUTS);
    let moved = windows.apply(7, PlasmaEvent::Geometry(Rect { x: 100, y: 100, width: 800, height: 600 }), OUTPUTS);

    let game = |activated, output: &str| Toplevel {
        app_id: "steam_app_238960".into(),
        title: "Path of Exile".into(),
        activated,
        outputs: vec![output.into()],
    };
    assert_eq!(deactivated, Some(ToplevelEvent::Updated { id: 7, toplevel: game(false, "HDMI-A-1") }));
    assert_eq!(moved, Some(ToplevelEvent::Updated { id: 7, toplevel: game(false, "DP-1") }));
}

#[test]
fn changes_that_move_neither_focus_nor_output_report_nothing() {
    let mut windows = PlasmaWindows::default();
    announce(&mut windows, 7, "steam_app_238960", "Path of Exile", ACTIVE);

    // KWin sends geometry continuously while a window is dragged.
    let dragged = windows.apply(7, PlasmaEvent::Geometry(Rect { x: 2000, y: 100, width: 800, height: 600 }), OUTPUTS);
    let fullscreened = windows.apply(7, PlasmaEvent::State(ACTIVE | FULLSCREEN), OUTPUTS);
    let retitled = windows.apply(7, PlasmaEvent::Title("Path of Exile".into()), OUTPUTS);

    assert_eq!((dragged, fullscreened, retitled), (None, None, None));
}

#[test]
fn an_unmapped_window_is_closed() {
    let mut windows = PlasmaWindows::default();
    announce(&mut windows, 7, "steam_app_238960", "Path of Exile", ACTIVE);

    assert_eq!(windows.apply(7, PlasmaEvent::Unmapped, OUTPUTS), Some(ToplevelEvent::Closed { id: 7 }));
    // Its object is gone; a late event about it reports nothing.
    assert_eq!(windows.apply(7, PlasmaEvent::State(0), OUTPUTS), None);
}

#[test]
fn the_overlay_follows_focus_from_the_game_to_firefox_and_back() {
    let mut windows = PlasmaWindows::default();
    let mut focus = GameFocus::default();
    let events = announce(&mut windows, 7, "steam_app_238960", "Path of Exile", ACTIVE)
        .into_iter()
        .chain(announce(&mut windows, 9, "org.mozilla.firefox", "poe.ninja — Mozilla Firefox", 0));
    events.for_each(|event| focus.apply(event));
    let mut set_state = |focus: &mut GameFocus, id, state| {
        if let Some(event) = windows.apply(id, PlasmaEvent::State(state), OUTPUTS) {
            focus.apply(event);
        }
    };

    // KWin moves the active flag: off the game first, then onto Firefox.
    set_state(&mut focus, 7, 0);
    set_state(&mut focus, 9, ACTIVE);
    let in_firefox = focus.visibility();
    set_state(&mut focus, 9, 0);
    set_state(&mut focus, 7, ACTIVE);

    assert_eq!(in_firefox, Visibility::Hidden);
    assert_eq!(focus.visibility(), Visibility::Shown { output: Some("HDMI-A-1".into()) });
}
