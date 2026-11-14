//! The overlay process: one per session, alive exactly as long as the game.

use std::os::fd::OwnedFd;
use std::process::ExitCode;

use gtk::prelude::*;
use gtk::{gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use crate::focus::Visibility;
use crate::{instance, toplevels};

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
    let game = match Pid::from_raw(game_pid).map(|pid| pidfd_open(pid, PidfdFlags::empty())) {
        Some(Ok(fd)) => fd,
        _ => {
            eprintln!("poe-overlay: game process {game_pid} is not running");
            return ExitCode::SUCCESS;
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
    let changes = toplevels::watch();
    glib::spawn_future_local(async move {
        while let Ok(visibility) = changes.recv().await {
            show(&window, &visibility);
        }
        window.set_visible(false);
    });
}

// Placeholder panel geometry, in surface-local logical pixels.
const PANEL_X: i32 = 40;
const PANEL_Y: i32 = 200;
const PANEL_W: i32 = 320;
const PANEL_H: i32 = 80;

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
    css.load_from_string(CSS);
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

    let panel = gtk::Box::new(gtk::Orientation::Vertical, 4);
    panel.add_css_class("panel");
    panel.set_size_request(PANEL_W, PANEL_H);
    panel.append(&gtk::Label::builder().label("PoE Overlay").xalign(0.0).build());
    let fixed = gtk::Fixed::new();
    fixed.put(&panel, PANEL_X as f64, PANEL_Y as f64);
    window.set_child(Some(&fixed));

    window.connect_realize(|window| {
        let surface = window.surface().expect("realized window has a surface");
        limit_input_to_panel(&surface);
        // Re-applied in case GTK recomputes the input region on relayout.
        surface.connect_layout(|surface, _, _| limit_input_to_panel(surface));
    });
    window
}

fn limit_input_to_panel(surface: &gdk::Surface) {
    let panel = cairo::RectangleInt::new(PANEL_X, PANEL_Y, PANEL_W, PANEL_H);
    surface.set_input_region(Some(&cairo::Region::create_rectangle(&panel)));
}

fn show(window: &gtk::ApplicationWindow, visibility: &Visibility) {
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
