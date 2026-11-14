//! Path of Exile 1 specifics.

/// X11 window class Proton gives the game window: `steam_app_<Steam app ID>`.
const STEAM_WINDOW_CLASS: &str = "steam_app_238960";

/// Whether a toplevel window is the Path of Exile 1 game window.
///
/// Standalone clients (Lutris, Heroic) are recognised by the game's own title.
pub fn is_game_window(app_id: &str, title: &str) -> bool {
    app_id == STEAM_WINDOW_CLASS || title == "Path of Exile"
}
