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
        // The overlay it starts must leave the player's settings, saved
        // progress, applications folder (KWin's entry) and keys alone.
        cmd.env("XDG_CONFIG_HOME", self.runtime_dir.path().join("config"))
            .env("XDG_STATE_HOME", self.runtime_dir.path().join("state"))
            .env("XDG_DATA_HOME", self.runtime_dir.path().join("data"))
            .env_remove("DISPLAY");
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

/// A Wine prefix holding only `user.reg`, as Wine writes its header.
fn wine_prefix(root: &Path, extra: &str) -> std::path::PathBuf {
    fs::create_dir_all(root).unwrap();
    let reg = root.join("user.reg");
    fs::write(
        &reg,
        format!(
            "WINE REGISTRY Version 2\n;; All keys relative to REGISTRY\\\\User\\\\S-1-5-21-0-0-1000-1000\n\n#arch=win64\n\n[Control Panel\\\\Mouse] 1793987668\n#time=1dd6e17de477456\n\"MouseSpeed\"=\"1\"\n{extra}"
        ),
    )
    .unwrap();
    reg
}

const STEAM_EXE_KEY: &str = "[Software\\\\Wine\\\\AppDefaults\\\\PathOfExileSteam.exe\\\\X11 Driver]\n\"GrabPointer\"=\"N\"\n";

#[test]
fn steam_launch_stops_wine_locking_the_cursor_to_the_game() {
    let launch = Launch::new();
    let compat = tempfile::tempdir().unwrap();
    let reg = wine_prefix(&compat.path().join("pfx"), "");

    let out = launch.command(&["true"]).env("STEAM_COMPAT_DATA_PATH", compat.path()).output().unwrap();

    assert!(out.status.success());
    let reg = fs::read_to_string(reg).unwrap();
    assert!(reg.contains(STEAM_EXE_KEY), "user.reg:\n{reg}");
}

#[test]
fn registry_fix_keeps_existing_settings_and_is_not_repeated() {
    let launch = Launch::new();
    let prefix = tempfile::tempdir().unwrap();
    let reg = wine_prefix(prefix.path(), "");
    let before = fs::read_to_string(&reg).unwrap();

    for _ in 0..2 {
        launch.command(&["true"]).env("WINEPREFIX", prefix.path()).output().unwrap();
    }

    let after = fs::read_to_string(&reg).unwrap();
    assert!(after.starts_with(&before), "existing content changed:\n{after}");
    assert_eq!(after.matches(STEAM_EXE_KEY).count(), 1, "user.reg:\n{after}");
}

#[test]
fn users_own_grab_pointer_choice_is_left_alone() {
    let launch = Launch::new();
    let prefix = tempfile::tempdir().unwrap();
    let own = "\n[Software\\\\Wine\\\\AppDefaults\\\\PathOfExileSteam.exe\\\\X11 Driver] 1794497090\n#time=1dd6e17de477456\n\"GrabPointer\"=\"Y\"\n";
    let reg = wine_prefix(prefix.path(), own);

    launch.command(&["true"]).env("WINEPREFIX", prefix.path()).output().unwrap();

    let after = fs::read_to_string(&reg).unwrap();
    assert!(after.contains(own), "user's value changed:\n{after}");
    assert!(!after.contains("PathOfExileSteam.exe\\\\X11 Driver]\n\"GrabPointer\"=\"N\""), "user.reg:\n{after}");
}

#[test]
fn registry_fix_joins_an_existing_section_for_the_game() {
    let launch = Launch::new();
    let prefix = tempfile::tempdir().unwrap();
    let section = "\n[Software\\\\Wine\\\\AppDefaults\\\\PathOfExileSteam.exe\\\\X11 Driver] 1794497090\n#time=1dd6e17de477456\n\"UseTakeFocus\"=\"N\"\n";
    let reg = wine_prefix(prefix.path(), section);

    launch.command(&["true"]).env("WINEPREFIX", prefix.path()).output().unwrap();

    let after = fs::read_to_string(&reg).unwrap();
    assert!(
        after.contains("PathOfExileSteam.exe\\\\X11 Driver] 1794497090\n#time=1dd6e17de477456\n\"GrabPointer\"=\"N\"\n\"UseTakeFocus\"=\"N\"\n"),
        "user.reg:\n{after}"
    );
    assert_eq!(after.matches("PathOfExileSteam.exe\\\\X11 Driver]").count(), 1);
}

/// What the AppImage's AppRun hands the program: the environment it was
/// started with, NUL-separated and base64-encoded, as `base64 -w0
/// /proc/$$/environ` prints it.
fn appimage_snapshot(vars: &[&str]) -> String {
    let mut raw = Vec::new();
    for var in vars {
        raw.extend_from_slice(var.as_bytes());
        raw.push(0);
    }
    let mut base64 = Command::new("base64").arg("-w0").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    std::io::Write::write_all(&mut base64.stdin.take().unwrap(), &raw).unwrap();
    String::from_utf8(base64.wait_with_output().unwrap().stdout).unwrap()
}

/// Sorted variables of `env -0` output.
fn environ_list(raw: &[u8]) -> Vec<String> {
    let mut vars: Vec<String> = raw.split(|b| *b == 0).filter(|v| !v.is_empty()).map(|v| String::from_utf8_lossy(v).into_owned()).collect();
    vars.sort();
    vars
}

#[test]
fn from_the_appimage_the_game_gets_the_environment_steam_gave_it() {
    let launch = Launch::new();
    let dir = tempfile::tempdir().unwrap();
    // Stands in for the AppImage file: records how it was started.
    let appimage = dir.path().join("poe-overlay.AppImage");
    fs::write(&appimage, format!("#!/bin/sh\necho \"$@\" > {0}/appimage-args\nenv -0 > {0}/appimage-env\n", dir.path().display())).unwrap();
    fs::set_permissions(&appimage, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let steam = [
        "PATH=/usr/bin:/bin",
        "LD_PRELOAD=/nonexistent/gameoverlayrenderer.so",
        "XDG_DATA_DIRS=/usr/local/share:/usr/share",
        "STEAM_COMPAT_DATA_PATH=/nonexistent/compatdata/238960",
        "SPACED=a value\nover two lines",
        // Encodes to base64's `+` and `/`.
        "SYMBOLS=~~~???~~~???",
    ];
    // The AppImage runtime adds these before AppRun takes its snapshot.
    let runtime = [
        "APPIMAGE=/home/exile/poe-overlay.AppImage",
        "APPDIR=/tmp/.mount_poe-oAbC12",
        "APPOFFSET=1504648",
        "ARGV0=poe-overlay.AppImage",
        "OWD=/home/exile",
        "URUNTIME=/home/exile/poe-overlay.AppImage",
        "URUNTIME_DIR=/home/exile",
    ];
    let snapshot = appimage_snapshot(&[&steam[..], &runtime[..]].concat());

    // The game prints its environment.
    let out = launch
        .command(&["/usr/bin/env", "-0"])
        .env("POE_OVERLAY_HOST_ENV", &snapshot)
        .env("APPIMAGE", &appimage)
        // What the bundled libraries need, which the game must not get.
        .env("GTK_PATH", "/tmp/.mount_poe-oAbC12/lib/gtk-4.0")
        .env("XDG_DATA_DIRS", "/tmp/.mount_poe-oAbC12/share:/usr/local/share:/usr/share")
        .env("LD_PRELOAD", "/tmp/.mount_poe-oAbC12/lib/anylinux.so")
        .output()
        .unwrap();

    let mut expected: Vec<String> = steam.iter().map(|v| v.to_string()).collect();
    expected.sort();
    assert_eq!(environ_list(&out.stdout), expected);
    // The overlay starts from the AppImage file, with Steam's environment
    // but without its overlay preload.
    let args = fs::read_to_string(dir.path().join("appimage-args")).unwrap();
    assert!(args.starts_with("overlay --game-pid "), "AppImage started with: {args}");
    let overlay_env = environ_list(&fs::read(dir.path().join("appimage-env")).unwrap());
    assert!(overlay_env.contains(&"XDG_DATA_DIRS=/usr/local/share:/usr/share".to_owned()), "{overlay_env:?}");
    assert!(!overlay_env.iter().any(|v| v.starts_with("LD_PRELOAD=") || v.starts_with("GTK_PATH=")), "{overlay_env:?}");
}
