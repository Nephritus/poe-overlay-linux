//! The `poe-overlay launch -- <game command>` contract Steam relies on.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// `poe-overlay launch -- <game...>` isolated to its own runtime dir, so it
/// never meets an overlay the user is running.
struct Launch {
    runtime_dir: tempfile::TempDir,
}

impl Launch {
    fn new() -> Self {
        Self { runtime_dir: tempfile::tempdir().unwrap() }
    }

    fn command(&self, game: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_poe-overlay"));
        cmd.arg("launch").arg("--").args(game);
        cmd.env("XDG_RUNTIME_DIR", self.runtime_dir.path());
        // The Wayland socket lives in the real runtime dir.
        if let (Some(dir), Some(display)) =
            (std::env::var_os("XDG_RUNTIME_DIR"), std::env::var_os("WAYLAND_DISPLAY"))
        {
            cmd.env("WAYLAND_DISPLAY", Path::new(&dir).join(display));
        }
        cmd
    }

    /// PIDs of overlay processes started for this launch's runtime dir.
    fn overlays(&self) -> Vec<u32> {
        let marker = format!("XDG_RUNTIME_DIR={}", self.runtime_dir.path().display());
        fs::read_dir("/proc")
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().to_str()?.parse::<u32>().ok())
            .filter(|pid| {
                let cmdline = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
                let args: Vec<&[u8]> = cmdline.split(|b| *b == 0).collect();
                args.len() > 1
                    && args[0].ends_with(b"poe-overlay")
                    && args[1] == b"overlay"
                    && environ(*pid).iter().any(|v| v == &marker)
            })
            .collect()
    }

    /// Waits until `cond` holds for the overlays, or panics after 3 s.
    fn wait_for_overlays(&self, what: &str, cond: impl Fn(&[u32]) -> bool) -> Vec<u32> {
        let start = Instant::now();
        loop {
            let pids = self.overlays();
            if cond(&pids) {
                return pids;
            }
            assert!(start.elapsed() < Duration::from_secs(3), "timed out waiting for {what}; overlays: {pids:?}");
            sleep(Duration::from_millis(50));
        }
    }

    fn run(&self, game: &[&str]) -> Output {
        self.command(game).output().unwrap()
    }
}

#[test]
fn launcher_exits_with_the_game_exit_code() {
    let out = Launch::new().run(&["sh", "-c", "exit 7"]);

    assert_eq!(out.status.code(), Some(7));
}

fn environ(pid: u32) -> Vec<String> {
    fs::read(format!("/proc/{pid}/environ"))
        .unwrap_or_default()
        .split(|b| *b == 0)
        .map(|v| String::from_utf8_lossy(v).into_owned())
        .collect()
}

fn needs_wayland() -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("skipped: needs a Wayland session");
        return false;
    }
    true
}

/// Stand-in for Steam's overlay injection; ld.so warns and carries on.
const STEAM_PRELOAD: &str = "/nonexistent/gameoverlayrenderer.so";

#[test]
fn game_receives_its_arguments_unchanged() {
    let out = Launch::new().run(&["sh", "-c", r#"printf '%s|' "$@""#, "sh", "-steam", "two words", ""]);

    assert_eq!(String::from_utf8_lossy(&out.stdout), "-steam|two words||");
}

#[test]
fn game_keeps_steam_environment_including_its_overlay_preload() {
    let launch = Launch::new();
    let out = launch
        .command(&["sh", "-c", r#"printf '%s' "$LD_PRELOAD""#])
        .env("LD_PRELOAD", STEAM_PRELOAD)
        .output()
        .unwrap();

    assert_eq!(String::from_utf8_lossy(&out.stdout), STEAM_PRELOAD);
}

#[test]
fn overlay_runs_alongside_the_game_without_steam_overlay_preload() {
    if !needs_wayland() {
        return;
    }
    let launch = Launch::new();
    let mut game = launch.command(&["sleep", "2"]).env("LD_PRELOAD", STEAM_PRELOAD).spawn().unwrap();

    let overlays = launch.wait_for_overlays("one overlay", |p| p.len() == 1);
    let env = environ(overlays[0]);
    game.wait().unwrap();

    assert!(!env.iter().any(|v| v.starts_with("LD_PRELOAD=")), "overlay env has LD_PRELOAD");
}

#[test]
fn overlay_exits_when_the_game_exits() {
    if !needs_wayland() {
        return;
    }
    let launch = Launch::new();
    let mut game = launch.command(&["sleep", "1"]).spawn().unwrap();
    launch.wait_for_overlays("overlay to start", |p| p.len() == 1);

    game.wait().unwrap();

    launch.wait_for_overlays("overlay to exit", |p| p.is_empty());
}

#[test]
fn launching_twice_runs_one_overlay() {
    if !needs_wayland() {
        return;
    }
    let launch = Launch::new();
    let mut first = launch.command(&["sleep", "2"]).spawn().unwrap();
    let mut second = launch.command(&["sleep", "2"]).spawn().unwrap();

    launch.wait_for_overlays("an overlay", |p| !p.is_empty());
    sleep(Duration::from_millis(700));
    let overlays = launch.overlays();
    first.wait().unwrap();
    second.wait().unwrap();

    assert_eq!(overlays.len(), 1, "overlays: {overlays:?}");
}
