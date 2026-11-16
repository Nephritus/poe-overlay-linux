use poe_overlay::focus::{GameFocus, Toplevel, ToplevelEvent, Visibility};

fn window(app_id: &str, title: &str, activated: bool, outputs: &[&str]) -> Toplevel {
    Toplevel {
        app_id: app_id.into(),
        title: title.into(),
        activated,
        outputs: outputs.iter().map(|o| o.to_string()).collect(),
    }
}

fn updated(id: u32, toplevel: Toplevel) -> ToplevelEvent {
    ToplevelEvent::Updated { id, toplevel }
}

#[test]
fn panel_is_shown_on_the_game_output_when_the_poe_window_is_activated() {
    let mut focus = GameFocus::default();

    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", true, &["DP-2"])));

    assert_eq!(focus.visibility(), Visibility::Shown { output: Some("DP-2".into()) });
}

#[test]
fn panel_hides_when_another_window_is_activated() {
    let mut focus = GameFocus::default();
    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", true, &["DP-2"])));

    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", false, &["DP-2"])));
    focus.apply(updated(2, window("firefox", "poe.ninja — Mozilla Firefox", true, &["DP-2"])));

    assert_eq!(focus.visibility(), Visibility::Hidden);
}

#[test]
fn no_window_focused_while_the_game_is_open_is_told_apart_from_another_window_focused() {
    // A layer surface holding the keyboard (the Overlay while the player
    // types in a Panel) leaves no window activated.
    let mut focus = GameFocus::default();
    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", true, &["DP-2"])));

    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", false, &["DP-2"])));

    assert_eq!(focus.visibility(), Visibility::NoWindowFocused { output: Some("DP-2".into()) });
}

#[test]
fn panel_returns_when_the_game_is_activated_again() {
    let mut focus = GameFocus::default();
    focus.apply(updated(2, window("firefox", "poe.ninja — Mozilla Firefox", true, &["DP-2"])));
    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", false, &["DP-2"])));

    focus.apply(updated(2, window("firefox", "poe.ninja — Mozilla Firefox", false, &["DP-2"])));
    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", true, &["DP-2"])));

    assert_eq!(focus.visibility(), Visibility::Shown { output: Some("DP-2".into()) });
}

#[test]
fn panel_hides_when_the_game_window_closes() {
    let mut focus = GameFocus::default();
    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", true, &["DP-2"])));

    focus.apply(ToplevelEvent::Closed { id: 1 });

    assert_eq!(focus.visibility(), Visibility::Hidden);
}

#[test]
fn standalone_client_is_recognised_by_its_window_title() {
    let mut focus = GameFocus::default();

    focus.apply(updated(1, window("pathofexile_x64.exe", "Path of Exile", true, &["eDP-1"])));

    assert_eq!(focus.visibility(), Visibility::Shown { output: Some("eDP-1".into()) });
}

#[test]
fn a_browser_tab_about_path_of_exile_is_not_the_game() {
    let mut focus = GameFocus::default();

    focus.apply(updated(1, window("firefox", "Path of Exile Trade — Mozilla Firefox", true, &["eDP-1"])));

    assert_eq!(focus.visibility(), Visibility::Hidden);
}

#[test]
fn panel_follows_the_game_to_another_output() {
    let mut focus = GameFocus::default();
    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", true, &["DP-2"])));

    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", true, &["HDMI-A-1"])));

    assert_eq!(focus.visibility(), Visibility::Shown { output: Some("HDMI-A-1".into()) });
}

#[test]
fn panel_is_shown_without_an_output_when_the_compositor_has_not_reported_one() {
    let mut focus = GameFocus::default();

    focus.apply(updated(1, window("steam_app_238960", "Path of Exile", true, &[])));

    assert_eq!(focus.visibility(), Visibility::Shown { output: None });
}
