//! `poe-overlay trigger <action>` reaching the running Overlay.

use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

use poe_overlay::running::{Action, RunningOverlay};

fn trigger(runtime_dir: &Path, action: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_poe-overlay"))
        .args(["trigger", action])
        .env("XDG_RUNTIME_DIR", runtime_dir)
        .output()
        .unwrap()
}

#[test]
fn trigger_delivers_the_action_to_the_running_overlay() {
    let dir = tempfile::tempdir().unwrap();
    let overlay = RunningOverlay::start_in(dir.path()).unwrap().expect("no other overlay");

    let out = trigger(dir.path(), "currency-rates");

    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let action = overlay.actions().recv_blocking().unwrap();
    assert_eq!(action, Action::CurrencyRates);
}

#[test]
fn trigger_without_a_running_overlay_fails_with_a_message() {
    let dir = tempfile::tempdir().unwrap();

    let out = trigger(dir.path(), "currency-rates");

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no overlay is running"), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn socket_left_by_a_crashed_overlay_is_not_mistaken_for_a_running_one() {
    let dir = tempfile::tempdir().unwrap();
    drop(UnixListener::bind(dir.path().join("poe-overlay.sock")).unwrap());

    let out = trigger(dir.path(), "currency-rates");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no overlay is running"));

    let overlay = RunningOverlay::start_in(dir.path()).unwrap().expect("stale socket blocked start");
    assert!(trigger(dir.path(), "currency-rates").status.success());
    assert_eq!(overlay.actions().recv_blocking().unwrap(), Action::CurrencyRates);
}

#[test]
fn second_overlay_does_not_start_while_one_runs() {
    let dir = tempfile::tempdir().unwrap();
    let _first = RunningOverlay::start_in(dir.path()).unwrap().expect("first overlay");

    assert!(RunningOverlay::start_in(dir.path()).unwrap().is_none());
}

#[test]
fn unknown_action_is_rejected_with_the_valid_ones() {
    let dir = tempfile::tempdir().unwrap();
    let _overlay = RunningOverlay::start_in(dir.path()).unwrap().expect("overlay");

    let out = trigger(dir.path(), "currency");

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("currency-rates"));
}

#[test]
fn trigger_gives_up_when_the_overlay_does_not_answer() {
    // A wedged overlay holding the socket must not hang the compositor's bind.
    let dir = tempfile::tempdir().unwrap();
    let _silent = UnixListener::bind(dir.path().join("poe-overlay.sock")).unwrap();

    let start = std::time::Instant::now();
    let out = trigger(dir.path(), "currency-rates");

    assert!(!out.status.success());
    assert!(start.elapsed() < Duration::from_secs(3));
}
