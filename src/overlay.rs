//! The overlay process: one per session, alive exactly as long as the game.

use std::os::fd::OwnedFd;
use std::process::ExitCode;

use gtk::{gio, glib};
use gtk::prelude::*;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::chat_sender::ChatSender;
use crate::config::{self, Config};
use crate::currency_panel::{self, CurrencyPanel};
use crate::focus::Visibility;
use crate::hotkeys::Hotkeys;
use crate::http::Http;
use crate::item_copier::ItemCopier;
use crate::key_grab::KeyGrabber;
use crate::overlay_surface::{OverlaySurface, PanelKind};
use crate::panel_prices::{Prices, Trades, ask};
use crate::poe1::client_log::{self, LogEvent};
use crate::poe1::leveling::{Leveling, ProgressStore};
use crate::poe1::prices::PriceSource;
use crate::poe1::route::Route;
use crate::poe1::trade_search::TradeSource;
use crate::price_panel::{self, PricePanel};
use crate::route_panel::{self, Notice, RoutePanel};
use crate::running::{Action, RunningOverlay};
use crate::settings_window;
use crate::trade_http::TradeSite;
use crate::trade_panel::{self, TradePanel};
use crate::uinput::LetterKeys;
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

    let overlay = OverlaySurface::new(app, &[route_panel::CSS, trade_panel::CSS]);
    let route_panel = RoutePanel::new();
    overlay.add_panel(route_panel.widget(), ROUTE_PANEL_X, ROUTE_PANEL_Y, PanelKind::Always);
    let settings_notice = route_panel.notice();
    // Before loading, so no change saved meanwhile is missed.
    let config_changes = config::watch();
    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("poe-overlay: {err}; using defaults");
            settings_notice.set(Some(&format!("Settings file has a mistake, using defaults: {err}")));
            Config::default()
        }
    };
    // Under the Toggled Panels, which the player opens on purpose.
    let letters = letter_keys();
    let chat = ChatSender::spawn(letters);
    let trade_panel = TradePanel::new(&overlay, chat.clone());
    // The Toggled Panels take turns in one spot next to the Route Panel.
    let currency_panel = CurrencyPanel::new();
    overlay.add_panel(currency_panel.widget(), TOGGLED_PANEL_X, TOGGLED_PANEL_Y, PanelKind::Toggled);
    let trades: Trades = Arc::new(Mutex::new(TradeSource::new(TradeSite::new())));
    if config.trade.login {
        trades.lock().unwrap().log_in(browser_login());
    }
    let price_panel = PricePanel::new((TOGGLED_PANEL_X, TOGGLED_PANEL_Y), trades.clone(), chat, &overlay);
    overlay.add_panel(price_panel.widget(), TOGGLED_PANEL_X, TOGGLED_PANEL_Y, PanelKind::Toggled);
    let held = config.hotkeys.get(Action::PriceCheck).map(|hotkey| hotkey.modifiers().collect()).unwrap_or_default();
    let copier = ItemCopier::spawn(letters, held);
    let prices: Prices = Arc::new(Mutex::new(PriceSource::new(Http::new(), config.prices.league.clone())));
    let log_follower = follow_log(route_panel.clone(), trade_panel, &config);
    let settings = Settings { league: config.prices.league, client_txt: config.game.client_txt, login: config.trade.login };
    follow_settings(config_changes, settings, settings_notice, (prices.clone(), trades), currency_panel.clone(), log_follower);
    let grabber = grab_hotkeys(route_panel, config.hotkeys, action_sender);
    take_actions(actions, overlay.clone(), currency_panel, price_panel, copier, prices);

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
/// the player presses Previous/Next (progress is saved per Character); Trade
/// Requests in Client.txt go to the Trade Requests Panel.
fn follow_log(panel: RoutePanel, trades: TradePanel, config: &Config) -> LogFollower {
    let route = Route::campaign(&config.route);
    for problem in route.problems() {
        eprintln!("poe-overlay: Route data: {problem}");
    }
    let (store, warning) = ProgressStore::open(ProgressStore::default_path());
    if let Some(warning) = warning {
        eprintln!("poe-overlay: {warning}");
        panel.set_note(&warning);
    }
    let log = poe1::client_log_path(config.game.client_txt.as_deref(), &steam::default_roots());
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

    let follower = LogFollower { notice: panel.notice(), panel, leveling, trades, lines: RefCell::default() };
    follower.watch(log);
    follower
}

/// Feeds one Client.txt at a time into the Leveling session and the Trade
/// Requests Panel.
struct LogFollower {
    panel: RoutePanel,
    leveling: Rc<RefCell<Leveling>>,
    trades: TradePanel,
    /// Says when there's no Client.txt to follow, or it can't be read.
    notice: Notice,
    /// Lines of the log being followed.
    lines: RefCell<Option<async_channel::Receiver<String>>>,
}

impl LogFollower {
    /// Follows another Client.txt from now on (one chosen in the settings).
    /// The `character` of its latest level-up is taken as a level-up would be.
    fn switch_to(&self, log: Option<PathBuf>, character: Option<String>) {
        if let Some(name) = character {
            let mut leveling = self.leveling.borrow_mut();
            leveling.handle(&LogEvent::CharacterLevelled { name });
            show(&self.panel, &leveling);
        }
        self.watch(log);
    }

    fn watch(&self, log: Option<PathBuf>) {
        // Ends the old log's reader and its thread.
        if let Some(old) = self.lines.borrow_mut().take() {
            old.close();
        }
        let Some(log) = log else {
            self.notice.set(Some("Client.txt not found in any Steam library, so the Route won't follow your Areas; choose it in `poe-overlay settings`."));
            return;
        };
        // Followed anyway: the file may appear once the game writes it.
        let problem = client_log::check_path(&log).err().map(|problem| format!("{problem} The Route follows your Areas once it can read it."));
        self.notice.set(problem.as_deref());
        let lines = log_follow::watch(log);
        *self.lines.borrow_mut() = Some(lines.clone());
        let (panel, leveling, trades) = (self.panel.clone(), self.leveling.clone(), self.trades.clone());
        glib::spawn_future_local(async move {
            while let Ok(line) = lines.recv().await {
                // Lines still queued from a log no longer followed.
                if lines.is_closed() {
                    return;
                }
                let Some(event) = client_log::parse_line(&line) else { continue };
                if let LogEvent::TradeRequest(request) = &event {
                    trades.add(request);
                    continue;
                }
                let mut leveling = leveling.borrow_mut();
                if leveling.handle(&event) {
                    show(&panel, &leveling);
                }
            }
        });
    }
}

/// The settings that change while the game runs; the rest of config.toml
/// applies at the next start.
struct Settings {
    league: Option<String>,
    client_txt: Option<PathBuf>,
    login: bool,
}

/// Reads the login from the player's browsers; why some couldn't be read
/// shows in the Settings window.
fn browser_login() -> poe1::login::ReadBrowsers {
    Box::new(|| poe1::login::from_browsers().0)
}

/// Applies changes to config.toml (from the Settings window or by hand) as
/// they're saved.
fn follow_settings(
    changes: async_channel::Receiver<Result<Config, String>>,
    mut current: Settings,
    notice: Notice,
    (prices, trades): (Prices, Trades),
    currency: CurrencyPanel,
    log_follower: LogFollower,
) {
    glib::spawn_future_local(async move {
        while let Ok(loaded) = changes.recv().await {
            let config = match loaded {
                Ok(config) => config,
                Err(err) => {
                    eprintln!("poe-overlay: {err}; keeping the settings in use");
                    notice.set(Some(&format!("Settings file has a mistake, keeping the settings in use: {err}")));
                    continue;
                }
            };
            notice.set(None);
            if config.game.client_txt != current.client_txt {
                current.client_txt = config.game.client_txt;
                let chosen = current.client_txt.clone();
                // Reads Steam's library list and, from the end, Client.txt (hundreds of MB).
                let found = gio::spawn_blocking(move || {
                    let log = poe1::client_log_path(chosen.as_deref(), &steam::default_roots());
                    let character = log.as_deref().and_then(client_log::last_character);
                    (log, character)
                })
                .await;
                if let Ok((log, character)) = found {
                    log_follower.switch_to(log, character);
                }
            }
            if config.prices.league != current.league {
                current.league = config.prices.league;
                let league = current.league.clone();
                let _ = ask(&prices, move |source| {
                    source.set_league(league);
                    Ok::<_, String>(())
                })
                .await;
                if currency.widget().is_visible() {
                    currency_panel::refresh(currency.clone(), prices.clone()).await;
                }
            }
            if config.trade.login != current.login {
                current.login = config.trade.login;
                let login = current.login;
                // Waits for a search under way.
                let _ = ask(&trades, move |source| {
                    if login { source.log_in(browser_login()) } else { source.log_out() }
                    Ok::<_, String>(())
                })
                .await;
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

/// Carries out Actions from hotkeys and triggers.
fn take_actions(
    actions: async_channel::Receiver<Action>,
    overlay: OverlaySurface,
    currency: CurrencyPanel,
    price: PricePanel,
    copier: ItemCopier,
    prices: Prices,
) {
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
                Action::Settings => settings_window::open(),
            }
        }
    });
}

/// The evdev codes of the keys that type `c` and `v` in the player's
/// keyboard layout, from the compositor's keymap; the QWERTY positions if GDK
/// doesn't know.
fn letter_keys() -> LetterKeys {
    let code = |letter: gtk::gdk::Key, qwerty: u16| {
        let keys = gtk::gdk::Display::default().and_then(|display| display.map_keyval(letter));
        let first_level = keys.and_then(|keys| keys.into_iter().find(|key| key.group() == 0 && key.level() == 0));
        // XKB key codes are evdev codes plus 8.
        first_level.and_then(|key| u16::try_from(key.keycode()).ok()?.checked_sub(8)).unwrap_or(qwerty)
    };
    LetterKeys { c: code(gtk::gdk::Key::c, 46), v: code(gtk::gdk::Key::v, 47) }
}

// Panel positions, in logical pixels from the output's top left.
const ROUTE_PANEL_X: f64 = 40.0;
const ROUTE_PANEL_Y: f64 = 200.0;
// Clear of the Route Panel (340 px plus padding and border).
const TOGGLED_PANEL_X: f64 = 420.0;
const TOGGLED_PANEL_Y: f64 = 200.0;
