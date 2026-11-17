//! KWin 6.0–6.7 hide KDE Plasma's window list
//! (`org_kde_plasma_window_management`) from a program unless an installed
//! desktop entry runs that program's executable and lists the interface under
//! `X-KDE-Wayland-Interfaces`. KWin 6.8 only hides it from sandboxed apps.

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::files;

/// The entry's file name: its own desktop ID, so it hides no packaged entry.
pub const ENTRY: &str = "io.github.nephritus.PoeOverlay.KWin.desktop";

/// [`allow_window_list`] for this program, in `$XDG_DATA_HOME/applications`.
/// Returns whether the entry was written now (KWin may take a moment to see
/// it); a failure is logged.
pub fn allow_this_program() -> bool {
    let (Some(data), Ok(exe)) = (crate::xdg::data_home(), std::env::current_exe()) else { return false };
    let desktop = std::env::var_os("XDG_CURRENT_DESKTOP");
    allow_window_list(desktop.as_deref(), &data.join("applications"), &exe).unwrap_or_else(|err| {
        eprintln!("poe-overlay: cannot ask KWin for its window list: {err}");
        false
    })
}

/// Under KDE Plasma (`current_desktop` is `$XDG_CURRENT_DESKTOP`), writes a
/// hidden desktop entry into `applications` letting `exe` read the window list.
/// Returns whether it was written now; it's left alone while it's up to date.
pub fn allow_window_list(current_desktop: Option<&OsStr>, applications: &Path, exe: &Path) -> io::Result<bool> {
    let kde = current_desktop.is_some_and(|desktops| desktops.as_encoded_bytes().split(|&b| b == b':').any(|d| d == b"KDE"));
    if !kde {
        return Ok(false);
    }
    let exe = exe.to_string_lossy();
    let mut text = String::from("[Desktop Entry]\nType=Application\nName=poe-overlay window tracking\n");
    // KWin compares the command's first word with the executable's path.
    if exe.contains(char::is_whitespace) {
        let _ = writeln!(text, "Exec=\"{exe}\"");
    } else {
        let _ = writeln!(text, "Exec={exe}");
    }
    text.push_str("NoDisplay=true\nX-KDE-Wayland-Interfaces=org_kde_plasma_window_management\n");
    let path = applications.join(ENTRY);
    if std::fs::read_to_string(&path).is_ok_and(|old| old == text) {
        return Ok(false);
    }
    files::write_atomic(&path, &text)?;
    Ok(true)
}
