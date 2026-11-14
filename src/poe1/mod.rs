//! Path of Exile 1 specifics.

pub mod client_log;
pub mod leveling;
pub mod prices;
pub mod route;

/// Game executables of the Steam and standalone clients.
pub const EXECUTABLES: &[&str] = &[
    "PathOfExileSteam.exe",
    "PathOfExile_x64Steam.exe",
    "PathOfExile.exe",
    "PathOfExile_x64.exe",
];

/// Whether a toplevel window is the Path of Exile 1 game window.
///
/// Under Proton its X11 window class is `steam_app_<Steam app ID>`;
/// standalone clients (Lutris, Heroic) are recognised by the game's own title.
pub fn is_game_window(app_id: &str, title: &str) -> bool {
    app_id.strip_prefix("steam_app_") == Some(STEAM_APP_ID) || title == "Path of Exile"
}

/// Steam app ID of Path of Exile 1.
const STEAM_APP_ID: &str = "238960";

/// The game's Client.txt in whichever Steam library under `steam_roots` has it.
pub fn client_log_path(steam_roots: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    crate::steam::app_install_dir(steam_roots, STEAM_APP_ID).map(|dir| dir.join("logs/Client.txt"))
}
