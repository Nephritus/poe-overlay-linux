//! KWin 6.0–6.7 give their window list only to a program whose desktop
//! entry asks for it; the overlay writes one for itself under KDE Plasma.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use poe_overlay::kwin_permission::{ENTRY, allow_window_list};

fn kde() -> Option<&'static OsStr> {
    Some(OsStr::new("KDE"))
}

fn entry(applications: &Path) -> String {
    fs::read_to_string(applications.join(ENTRY)).unwrap()
}

#[test]
fn under_kde_a_hidden_entry_asks_for_the_window_list_for_this_program() {
    let home = tempfile::tempdir().unwrap();
    let applications = home.path().join(".local/share/applications");

    let written = allow_window_list(kde(), &applications, Path::new("/home/exile/.cargo/bin/poe-overlay")).unwrap();

    assert!(written);
    let text = entry(&applications);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "[Desktop Entry]");
    for line in [
        "Type=Application",
        "Exec=/home/exile/.cargo/bin/poe-overlay",
        "NoDisplay=true",
        "X-KDE-Wayland-Interfaces=org_kde_plasma_window_management",
    ] {
        assert!(lines.contains(&line), "{line} missing from:\n{text}");
    }
}

#[test]
fn the_entry_is_rewritten_only_when_the_program_moved() {
    let home = tempfile::tempdir().unwrap();
    let applications = home.path().join("applications");
    // An AppImage runs from a new mount each time.
    let first = Path::new("/tmp/.mount_poe-oAbC12/usr/bin/poe-overlay");
    let next = Path::new("/tmp/.mount_poe-oXyZ34/usr/bin/poe-overlay");
    allow_window_list(kde(), &applications, first).unwrap();

    let again = allow_window_list(kde(), &applications, first).unwrap();
    let moved = allow_window_list(kde(), &applications, next).unwrap();

    assert!(!again);
    assert!(moved);
    assert!(entry(&applications).contains("\nExec=/tmp/.mount_poe-oXyZ34/usr/bin/poe-overlay\n"));
}

#[test]
fn a_path_with_spaces_is_quoted() {
    let home = tempfile::tempdir().unwrap();

    allow_window_list(kde(), home.path(), Path::new("/home/exile/My Games/poe-overlay")).unwrap();

    assert!(entry(home.path()).contains("\nExec=\"/home/exile/My Games/poe-overlay\"\n"));
}

#[test]
fn other_desktops_get_no_entry() {
    let home = tempfile::tempdir().unwrap();
    let exe = Path::new("/usr/bin/poe-overlay");

    for desktop in [None, Some("niri"), Some("GNOME"), Some("Hyprland"), Some("sway:wlroots")] {
        assert!(!allow_window_list(desktop.map(OsStr::new), home.path(), exe).unwrap());
    }

    assert!(!home.path().join(ENTRY).exists());
    // XDG_CURRENT_DESKTOP is a list.
    assert!(allow_window_list(Some(OsStr::new("Custom:KDE")), home.path(), exe).unwrap());
}
