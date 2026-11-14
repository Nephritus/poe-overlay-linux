//! The overlay process: one per session, alive exactly as long as the game.

use std::os::fd::OwnedFd;
use std::process::ExitCode;

use gtk::prelude::*;
use gtk::{gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use std::cell::Cell;

use crate::config::Config;
use crate::focus::Visibility;
use crate::poe1::client_log::{self, LogEvent};
use crate::poe1::route::{Route, RouteProgress};
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

    let window = overlay_window(app);
    let route_panel = RoutePanel::new();
    let fixed = gtk::Fixed::new();
    fixed.put(route_panel.widget(), PANEL_X, PANEL_Y);
    window.set_child(Some(&fixed));
    accept_input_only_on(&window, route_panel.widget().clone().upcast());
    follow_route(route_panel);

    let changes = toplevels::watch();
    glib::spawn_future_local(async move {
        while let Ok(visibility) = changes.recv().await {
            apply_visibility(&window, &visibility);
        }
        window.set_visible(false);
    });
}

/// Shows the Route and moves it along as Client.txt reports Area changes.
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
    let mut progress = RouteProgress::new(route);
    panel.show(&progress);
    let Some(log) = poe1::client_log_path(&steam::default_roots()) else {
        panel.set_note("Client.txt not found in any Steam library; the Route won't follow your Areas.");
        return;
    };
    let lines = log_follow::watch(log);
    glib::spawn_future_local(async move {
        while let Ok(line) = lines.recv().await {
            if let Some(LogEvent::AreaEntered { area_id }) = client_log::parse_line(&line)
                && progress.enter_area(&area_id)
            {
                panel.show(&progress);
            }
        }
    });
}

// Route panel position, in surface-local logical pixels.
const PANEL_X: f64 = 40.0;
const PANEL_Y: f64 = 200.0;

const CSS: &str = "
window.poe-overlay { background: transparent; }
.panel {
  background: rgba(20, 20, 30, 0.8);
  border: 1px solid rgb(204, 153, 51);
  border-radius: 8px;
  padding: 12px;
  color: white;
}
";

/// A full-screen, transparent, never-focused layer surface on the Overlay
/// layer. Only panels accept pointer input; everything else passes through.
fn overlay_window(app: &gtk::Application) -> gtk::ApplicationWindow {
    let css = gtk::CssProvider::new();
    css.load_from_string(&format!("{CSS}{}", route_panel::CSS));
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("GDK display"),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let window = gtk::ApplicationWindow::new(app);
    window.add_css_class("poe-overlay");
    window.set_decorated(false);
    window.init_layer_shell();
    window.set_namespace(Some("poe-overlay"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_exclusive_zone(-1);
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(edge, true);
    }
    window
}

/// Limits pointer input to `panel`'s current bounds, re-checked after every
/// repaint because the panel resizes with its content.
fn accept_input_only_on(window: &gtk::ApplicationWindow, panel: gtk::Widget) {
    let window_ref = window.clone();
    window.connect_realize(move |_| {
        let surface = window_ref.surface().expect("realized window has a surface");
        let applied = Cell::new(None);
        let window = window_ref.clone();
        let panel = panel.clone();
        let update = move || {
            let Some(bounds) = panel.compute_bounds(&window) else { return };
            let rect = cairo::RectangleInt::new(
                bounds.x().floor() as i32,
                bounds.y().floor() as i32,
                bounds.width().ceil() as i32,
                bounds.height().ceil() as i32,
            );
            if applied.get() != Some(rect) {
                surface.set_input_region(Some(&cairo::Region::create_rectangle(&rect)));
                applied.set(Some(rect));
            }
        };
        update();
        window_ref.frame_clock().expect("realized window has a frame clock").connect_after_paint(move |_| update());
    });
}

fn apply_visibility(window: &gtk::ApplicationWindow, visibility: &Visibility) {
    let Visibility::Shown { output } = visibility else {
        window.set_visible(false);
        return;
    };
    let monitor = output.as_deref().and_then(monitor_named);
    if window.is_visible() && window.monitor() == monitor {
        return;
    }
    // A layer surface only moves to another output when remapped.
    window.set_visible(false);
    window.set_monitor(monitor.as_ref());
    window.present();
}

fn monitor_named(connector: &str) -> Option<gdk::Monitor> {
    gdk::Display::default()?
        .monitors()
        .iter::<gdk::Monitor>()
        .flatten()
        .find(|m| m.connector().as_deref() == Some(connector))
}
