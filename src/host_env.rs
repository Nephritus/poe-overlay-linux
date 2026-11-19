//! Inside the AppImage, the environment the player's session (Steam) started
//! it with. The AppImage points variables such as `GTK_PATH`, `XDG_DATA_DIRS`
//! and `LD_PRELOAD` into its own folder for the bundled libraries; the
//! programs the overlay starts (the game, the browser, itself again) must get
//! the session's instead. AppRun hands it over as `POE_OVERLAY_HOST_ENV`:
//! `/proc/$$/environ` (NUL-separated) in base64, taken before anything changes.

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::process::Command;
use std::sync::LazyLock;

use gtk::gio;
use gtk::prelude::*;

use crate::base64;

const SNAPSHOT: &str = "POE_OVERLAY_HOST_ENV";

/// Set by the AppImage runtime (uruntime) before AppRun runs, not by the
/// session.
const RUNTIME_VARS: [&[u8]; 7] = [b"APPIMAGE", b"APPDIR", b"APPOFFSET", b"ARGV0", b"OWD", b"URUNTIME", b"URUNTIME_DIR"];

/// The session's environment, or `None` outside the AppImage.
fn host() -> Option<&'static [(OsString, OsString)]> {
    static HOST: LazyLock<Option<Vec<(OsString, OsString)>>> = LazyLock::new(|| {
        let snapshot = std::env::var(SNAPSHOT).ok()?;
        let Some(raw) = base64::decode(&snapshot) else {
            eprintln!("poe-overlay: {SNAPSHOT} isn't base64; programs it starts get the AppImage's environment");
            return None;
        };
        let vars = raw.split(|&byte| byte == 0).filter_map(|var| {
            let eq = var.iter().position(|&byte| byte == b'=')?;
            let (name, value) = (&var[..eq], &var[eq + 1..]);
            (!RUNTIME_VARS.contains(&name)).then(|| (OsStr::from_bytes(name).to_owned(), OsStr::from_bytes(value).to_owned()))
        });
        Some(vars.collect())
    });
    HOST.as_deref()
}

/// A command for `program` with the session's environment.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    if let Some(host) = host() {
        command.env_clear().envs(host.iter().map(|(name, value)| (name, value)));
    }
    command
}

/// Runs this program again: the AppImage file inside the AppImage (its mount
/// goes away with the process that made it), else this executable.
pub fn own_command() -> io::Result<Command> {
    match (host(), std::env::var_os("APPIMAGE")) {
        (Some(_), Some(appimage)) => Ok(command(appimage)),
        _ => Ok(command(std::env::current_exe()?)),
    }
}

/// For starting a program through GIO (the default browser); `None` outside
/// the AppImage.
pub fn launch_context() -> Option<gio::AppLaunchContext> {
    let host = host()?;
    let context = gio::AppLaunchContext::new();
    for (name, _) in std::env::vars_os() {
        if !host.iter().any(|(host_name, _)| *host_name == name) {
            context.unsetenv(name);
        }
    }
    for (name, value) in host {
        context.setenv(name, value);
    }
    Some(context)
}
