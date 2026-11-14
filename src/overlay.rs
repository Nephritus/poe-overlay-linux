//! The overlay process: one per session, alive exactly as long as the game.

use std::os::fd::OwnedFd;
use std::process::ExitCode;

use gtk::glib;
use gtk::prelude::*;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::currency_panel::{self, CurrencyPanel};
use crate::focus::Visibility;
use crate::hotkeys::Hotkeys;
use crate::http::Http;
use crate::item_copier::ItemCopier;
use crate::key_grab::KeyGrabber;
use crate::overlay_surface::{OverlaySurface, PanelKind};
use crate::poe1::client_log;
use crate::poe1::leveling::{Leveling, ProgressStore};
use crate::poe1::prices::PriceSource;
use crate::poe1::route::Route;
use crate::price_panel::{self, PricePanel};
use crate::route_panel::{self, RoutePanel};
use crate::running::{Action, RunningOverlay};
use crate::{log_follow, poe1, steam, toplevels};

const APP_ID: &str = "io.github.nephritus.PoeOverlay";

pub fn run(game_pid: i32) -> ExitCode {
    let running = match RunningOverlay::start() {
        Ok(Some(running)) => running,
        Ok(None) => {
            eprintln!("poe-overlay: an overlay is already running");
            return ExitCode::SUCCESS;
        }
        Err(err) => {
            eprintln!("poe-overlay: cannot start: {err}");
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
        // Single-instance is RunningOverlay's job; D-Bus uniqueness would hand
        // activation to an unrelated running overlay.
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let (actions, action_sender) = (running.actions(), running.sender());
    app.connect_activate(move |app| activate(app, game_exited.clone(), actions.clone(), action_sender.clone()));
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

fn activate(
    app: &gtk::Application,
    game_exited: async_channel::Receiver<()>,
    actions: async_channel::Receiver<Action>,
    action_sender: async_channel::Sender<Action>,
) {
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
    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("poe-overlay: {err}; using defaults");
            route_panel.set_note(&format!("Settings file has a mistake, using defaults: {err}"));
            Config::default()
        }
    };
    // The Toggled Panels take turns in one spot next to the Route Panel.
    let currency_panel = CurrencyPanel::new();
    overlay.add_panel(currency_panel.widget(), TOGGLED_PANEL_X, TOGGLED_PANEL_Y, PanelKind::Toggled);
    let price_panel = PricePanel::new((TOGGLED_PANEL_X, TOGGLED_PANEL_Y));
    overlay.add_panel(price_panel.widget(), TOGGLED_PANEL_X, TOGGLED_PANEL_Y, PanelKind::Toggled);
    let held = config.hotkeys.get(Action::PriceCheck).map(|hotkey| hotkey.modifiers().collect()).unwrap_or_default();
    let copier = ItemCopier::spawn(key_code_of_c(), held);
    follow_route(route_panel.clone(), &config);
    let grabber = grab_hotkeys(route_panel, config.hotkeys, action_sender);
    take_actions(actions, overlay.clone(), currency_panel, price_panel, copier, config.prices.league);

    let changes = toplevels::watch();
    glib::spawn_future_local(async move {
        while let Ok(visibility) = changes.recv().await {
            grabber.follow_game(matches!(visibility, Visibility::Shown { .. }));
            overlay.follow_game(&visibility);
        }
        grabber.follow_game(false);
        overlay.follow_game(&Visibility::Hidden);
    });
}

/// Shows the Route and moves it along as Client.txt reports Area changes and
/// the player presses Previous/Next; progress is saved per Character.
fn follow_route(panel: RoutePanel, config: &Config) {
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

/// Grabs the hotkeys while the game is focused; problems become notes on
/// the Route Panel.
fn grab_hotkeys(panel: RoutePanel, hotkeys: Hotkeys, actions: async_channel::Sender<Action>) -> KeyGrabber {
    let (grabber, notes) = KeyGrabber::spawn(hotkeys, actions);
    glib::spawn_future_local(async move {
        while let Ok(note) = notes.recv().await {
            panel.set_note(&note);
        }
    });
    grabber
}

fn show(panel: &RoutePanel, leveling: &Leveling) {
    panel.show(leveling.progress(), leveling.character());
}

/// Carries out Actions from hotkeys while the game is focused.
fn take_actions(
    actions: async_channel::Receiver<Action>,
    overlay: OverlaySurface,
    currency: CurrencyPanel,
    price: PricePanel,
    copier: ItemCopier,
    league: Option<String>,
) {
    let prices = Arc::new(Mutex::new(PriceSource::new(Http::new(), league)));
    glib::spawn_future_local(async move {
        while let Ok(action) = actions.recv().await {
            // Not awaited here: a slow copy or fetch mustn't hold up the next Action.
            match action {
                Action::CurrencyRates => {
                    if overlay.toggle(currency.widget()) {
                        glib::spawn_future_local(currency_panel::refresh(currency.clone(), prices.clone()));
                    }
                }
                Action::PriceCheck => {
                    let check = price_panel::price_check(copier.clone(), overlay.clone(), price.clone(), prices.clone());
                    glib::spawn_future_local(check);
                }
            }
        }
    });
}

/// The evdev code of the key that types `c` in the player's keyboard layout,
/// from the compositor's keymap; the QWERTY position if GDK doesn't know.
fn key_code_of_c() -> u16 {
    const QWERTY_C: u16 = 46;
    let keys = gtk::gdk::Display::default().and_then(|display| display.map_keyval(gtk::gdk::Key::c));
    let first_level = keys.and_then(|keys| keys.into_iter().find(|key| key.group() == 0 && key.level() == 0));
    // XKB key codes are evdev codes plus 8.
    first_level.and_then(|key| u16::try_from(key.keycode()).ok()?.checked_sub(8)).unwrap_or(QWERTY_C)
}

// Panel positions, in logical pixels from the output's top left.
const ROUTE_PANEL_X: f64 = 40.0;
const ROUTE_PANEL_Y: f64 = 200.0;
// Clear of the Route Panel (340 px plus padding and border).
const TOGGLED_PANEL_X: f64 = 420.0;
const TOGGLED_PANEL_Y: f64 = 200.0;
