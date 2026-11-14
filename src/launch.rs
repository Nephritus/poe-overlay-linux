//! `poe-overlay launch -- <game>`: the Steam launch option entry point.

use std::ffi::OsString;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode, Stdio};

/// Starts the overlay, then replaces this process with the game so Steam sees
/// the game's own PID, arguments, environment and exit status. The overlay
/// follows that PID and exits with the game. Returns only if exec fails.
pub fn run(game: &[OsString]) -> ExitCode {
    if let Err(err) = spawn_overlay(std::process::id()) {
        eprintln!("poe-overlay: overlay not started: {err}");
    }
    let err = Command::new(&game[0]).args(&game[1..]).exec();
    eprintln!("poe-overlay: cannot start {:?}: {err}", game[0]);
    ExitCode::from(127)
}

/// Starts `poe-overlay overlay --game-pid <pid>` detached from this process
/// tree, so it never becomes a child of the game or its runtime container.
fn spawn_overlay(game_pid: u32) -> io::Result<()> {
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.args(["overlay", "--game-pid", &game_pid.to_string()])
        // Steam injects its in-game overlay renderer via LD_PRELOAD; it must
        // not hook the overlay's own rendering.
        .env_remove("LD_PRELOAD")
        .stdin(Stdio::null());
    // SAFETY: only async-signal-safe calls between fork and exec.
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
