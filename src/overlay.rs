//! The overlay process: one per session, alive exactly as long as the game.

use std::os::fd::OwnedFd;
use std::process::ExitCode;

use gtk::glib;
use gtk::prelude::*;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use std::cell::RefCell;
use std::rc::Rc;

use crate::config::Config;
use crate::focus::Visibility;
use crate::poe1::client_log;
use crate::poe1::leveling::{Leveling, ProgressStore};
use crate::poe1::route::Route;
use crate::overlay_surface::{OverlaySurface, PanelKind};
use crate::route_panel::{self, RoutePanel};
use crate::{instance, log_follow, poe1, steam, toplevels};

const APP_ID: &str = "io.github.nephritus.PoeOverlay";

pub fn run(game_pid: i32) -> ExitCode {
    let _lock = match instance::acquire() {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            eprintln!("poe-overlay: an overlay is already running");
            return ExitCode::SUCCESS;
        }
        Err(err) => {
            eprintln!("poe-overlay: cannot take the instance lock: {err}");
            return ExitCode::FAILURE;
        }
    };
    let Some(pid) = Pid::from_raw(game_pid) else {
        eprintln!("poe-overlay: invalid game PID {game_pid}");
        return ExitCode::FAILURE;
    };
    let game = match pidfd_open(pid, PidfdFlags::empty()) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::SRCH) => {
            eprintln!("poe-overlay: game process {game_pid} already exited");
            return ExitCode::SUCCESS;
        }
        Err(err) => {
            eprintln!("poe-overlay: cannot watch game process {game_pid}: {err}");
            return ExitCode::FAILURE;
        }
    };
    let game_exited = exit_signal(game);

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        // Single-instance is the lock's job; D-Bus uniqueness would hand
        // activation to an unrelated running overlay.
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| activate(app, game_exited.clone()));
    // GTK must not parse our CLI arguments.
    app.run_with_args::<&str>(&[]);
    ExitCode::SUCCESS
}

/// Receives once the process behind `pidfd` exits (a pidfd polls readable then).
fn exit_signal(pidfd: OwnedFd) -> async_channel::Receiver<()> {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let mut fds = [PollFd::new(&pidfd, PollFlags::IN)];
        while poll(&mut fds, None).is_err() {}
        let _ = tx.send_blocking(());
    });
    rx
}

fn activate(app: &gtk::Application, game_exited: async_channel::Receiver<()>) {
    let hold = app.hold();
    {
        let app = app.clone();
        glib::spawn_future_local(async move {
            let _ = game_exited.recv().await;
            drop(hold);
            app.quit();
        });
    }

    let overlay = OverlaySurface::new(app, &[route_panel::CSS]);
    let route_panel = RoutePanel::new();
    overlay.add_panel(route_panel.widget(), ROUTE_PANEL_X, ROUTE_PANEL_Y, PanelKind::Always);
    follow_route(route_panel);

    let changes = toplevels::watch();
    glib::spawn_future_local(async move {
        while let Ok(visibility) = changes.recv().await {
            overlay.follow_game(&visibility);
        }
        overlay.follow_game(&Visibility::Hidden);
    });
}

/// Shows the Route and moves it along as Client.txt reports Area changes and
/// the player presses Previous/Next; progress is saved per Character.
fn follow_route(panel: RoutePanel) {
    let config = Config::load().unwrap_or_else(|err| {
        eprintln!("poe-overlay: {err}; using defaults");
        panel.set_note(&format!("Settings file has a mistake, using defaults: {err}"));
        Config::default()
    });
    let route = Route::campaign(&config.route);
    for problem in route.problems() {
        eprintln!("poe-overlay: Route data: {problem}");
    }
    let (store, warning) = ProgressStore::open(ProgressStore::default_path());
    if let Some(warning) = warning {
        eprintln!("poe-overlay: {warning}");
        panel.set_note(&warning);
    }
    let log = poe1::client_log_path(&steam::default_roots());
    let assumed = log.as_deref().and_then(client_log::last_character);
    let leveling = Rc::new(RefCell::new(Leveling::new(route, store, assumed)));
    show(&panel, &leveling.borrow());

    let on_click = |step: fn(&mut Leveling) -> bool| {
        let (leveling, panel) = (leveling.clone(), panel.clone());
        move || {
            let mut leveling = leveling.borrow_mut();
            if step(&mut leveling) {
                show(&panel, &leveling);
            }
        }
    };
    panel.on_previous(on_click(Leveling::previous_leg));
    panel.on_next(on_click(Leveling::next_leg));

    let Some(log) = log else {
        panel.set_note("Client.txt not found in any Steam library; the Route won't follow your Areas.");
        return;
    };
    let lines = log_follow::watch(log);
    glib::spawn_future_local(async move {
        while let Ok(line) = lines.recv().await {
            let Some(event) = client_log::parse_line(&line) else { continue };
            let mut leveling = leveling.borrow_mut();
            if leveling.handle(&event) {
                show(&panel, &leveling);
            }
        }
    });
}

fn show(panel: &RoutePanel, leveling: &Leveling) {
    panel.show(leveling.progress(), leveling.character());
}

// Panel positions, in logical pixels from the output's top left.
const ROUTE_PANEL_X: f64 = 40.0;
const ROUTE_PANEL_Y: f64 = 200.0;
