//! `poe-overlay launch -- <game>`: the Steam launch option entry point.

use std::ffi::OsString;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{ExitCode, Stdio};

use crate::{host_env, poe1, wine};

/// Prepares the game's Wine prefix and starts the overlay, then replaces this
/// process with the game so Steam sees the game's own PID, arguments,
/// environment and exit status (inside the AppImage, the environment it was
/// started with). The overlay follows that PID and exits with the game.
/// Returns only if exec fails.
pub fn run(game: &[OsString]) -> ExitCode {
    if let Some(prefix) = wine::prefix_from_env()
        && let Err(err) = wine::keep_cursor_unlocked(&prefix, poe1::EXECUTABLES)
    {
        eprintln!("poe-overlay: cursor lock fix not applied to {}: {err}", prefix.display());
    }
    if let Err(err) = spawn_overlay(std::process::id()) {
        eprintln!("poe-overlay: overlay not started: {err}");
    }
    let err = host_env::command(&game[0]).args(&game[1..]).exec();
    eprintln!("poe-overlay: cannot start {:?}: {err}", game[0]);
    ExitCode::from(127)
}

/// Starts `poe-overlay overlay --game-pid <pid>` detached (new session, orphaned
/// to the nearest subreaper, Steam's reaper under Steam), so it never becomes a
/// child of the game or its runtime container. Steam still counts it as part of
/// the game, which is why it must exit with the game.
fn spawn_overlay(game_pid: u32) -> io::Result<()> {
    let mut cmd = host_env::own_command()?;
    cmd.args(["overlay", "--game-pid", &game_pid.to_string()])
        // Steam injects its in-game overlay renderer via LD_PRELOAD; it must
        // not hook the overlay's own rendering.
        .env_remove("LD_PRELOAD")
        .stdin(Stdio::null());
    // SAFETY: the launcher is still single-threaded here, so forking again in
    // the child before exec cannot deadlock on locks held by other threads.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            match libc::fork() {
                -1 => Err(io::Error::last_os_error()),
                0 => Ok(()),
                _ => libc::_exit(0),
            }
        });
    }
    // Reap the intermediate process; the overlay itself is reparented.
    cmd.spawn()?.wait()?;
    Ok(())
}
