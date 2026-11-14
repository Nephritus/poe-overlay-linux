//! Path of Exile 1 specifics.

/// Steam app ID of Path of Exile 1.
pub const STEAM_APP_ID: u32 = 238960;

/// Whether a toplevel window is the Path of Exile 1 game window.
///
/// Under Proton the X11 window class is `steam_app_<id>`; other launchers keep
/// the game's own window title.
pub fn is_game_window(app_id: &str, title: &str) -> bool {
    app_id == format!("steam_app_{STEAM_APP_ID}") || title == "Path of Exile"
}
