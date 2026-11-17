//! The Settings window: a normal window in a process of its own, where the
//! player types and chooses settings (the Overlay takes the keyboard only for
//! a Price Check bound). Saving writes config.toml, which a running overlay
//! re-reads within a second; so do Log in and Log out, at once.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::config::{self, Config, ConfigFile};
use crate::http::Http;
use crate::poe1::client_log;
use crate::poe1::login::{self, Login};
use crate::poe1::prices::PriceSource;
use crate::{files, poe1, steam};

const APP_ID: &str = "io.github.nephritus.PoeOverlay.Settings";

/// `poe-overlay settings`. A second start shows the open window instead.
pub fn run() -> ExitCode {
    let app = gtk::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| match app.active_window() {
        Some(window) => window.present(),
        None => SettingsWindow::build(app),
    });
    // GTK must not parse our CLI arguments.
    app.run_with_args::<&str>(&[]);
    ExitCode::SUCCESS
}

/// Starts `poe-overlay settings` from the overlay: a window of its own.
pub fn open() {
    let started = std::env::current_exe().and_then(|program| std::process::Command::new(program).arg("settings").spawn());
    match started {
        // Reaped on a thread, so the overlay never waits for the window.
        Ok(mut window) => drop(std::thread::spawn(move || window.wait())),
        Err(err) => eprintln!("poe-overlay: cannot open the settings window: {err}"),
    }
}

/// The league dropdown's entries; `None` is poe.ninja's first league.
type Leagues = Rc<RefCell<Vec<Option<Arc<str>>>>>;

#[derive(Clone)]
struct SettingsWindow {
    window: gtk::ApplicationWindow,
    league: gtk::DropDown,
    league_names: gtk::StringList,
    leagues: Leagues,
    client_txt: gtk::Entry,
    path_problem: gtk::Label,
    status: gtk::Label,
    login_status: gtk::Label,
    login_button: gtk::Button,
    logged_in: Rc<Cell<bool>>,
    /// Counts the times the login state was shown, so a search for the
    /// login stops once a newer one starts (after Log in or Log out).
    login_shown: Rc<Cell<u32>>,
}

impl SettingsWindow {
    fn build(app: &gtk::Application) {
        let window = gtk::ApplicationWindow::builder().application(app).title("poe-overlay settings").default_width(560).default_height(300).build();
        let grid = gtk::Grid::builder().row_spacing(6).column_spacing(12).margin_top(18).margin_bottom(18).margin_start(18).margin_end(18).build();

        let league_names = gtk::StringList::new(&[]);
        let league = gtk::DropDown::builder().model(&league_names).hexpand(true).build();
        grid.attach(&label("League"), 0, 0, 1, 1);
        grid.attach(&league, 1, 0, 2, 1);
        let league_status = hint("Currency Rates and Price Check show poe.ninja's prices in this league.");
        grid.attach(&league_status, 1, 1, 2, 1);

        let client_txt = gtk::Entry::builder().hexpand(true).placeholder_text("Found through Steam").build();
        let choose = gtk::Button::with_label("Choose…");
        grid.attach(&label("Client.txt"), 0, 2, 1, 1);
        grid.attach(&client_txt, 1, 2, 1, 1);
        grid.attach(&choose, 2, 2, 1, 1);
        grid.attach(&hint(&steam_hint()), 1, 3, 2, 1);
        let path_problem = hint("");
        path_problem.set_css_classes(&["error"]);
        path_problem.set_visible(false);
        grid.attach(&path_problem, 1, 4, 2, 1);

        let login_status = hint("");
        login_status.set_hexpand(true);
        let login_button = gtk::Button::with_label("Log in");
        login_button.set_valign(gtk::Align::Start);
        grid.attach(&label("Trade site"), 0, 5, 1, 1);
        grid.attach(&login_status, 1, 5, 1, 1);
        grid.attach(&login_button, 2, 5, 1, 1);
        grid.attach(
            &hint("Log in on pathofexile.com in your browser, and searches carry that login: the overlay reads the browser's pathofexile.com cookies, no other site's. Log out makes the overlay forget it; your browser stays logged in."),
            1,
            6,
            2,
            1,
        );

        let save = gtk::Button::with_label("Save");
        save.add_css_class("suggested-action");
        save.set_halign(gtk::Align::End);
        save.set_valign(gtk::Align::Start);
        let status = hint("");
        status.set_hexpand(true);
        grid.attach(&status, 0, 7, 2, 1);
        grid.attach(&save, 2, 7, 1, 1);
        window.set_child(Some(&grid));

        let this = SettingsWindow {
            window,
            league,
            league_names,
            leagues: Leagues::default(),
            client_txt,
            path_problem,
            status,
            login_status,
            login_button,
            logged_in: Rc::default(),
            login_shown: Rc::default(),
        };
        let file = config::path().ok_or_else(|| "No HOME folder, so there's nowhere to save settings.".to_owned());
        let current = file.as_deref().map_err(Clone::clone).and_then(read);
        match &current {
            Ok(current) => {
                this.client_txt.set_text(&current.client_txt().map(|path| path.to_string_lossy()).unwrap_or_default());
                this.fill_leagues(&[], current.league());
                this.show_login(current.login(), false);
            }
            Err(err) => {
                // Saving would replace what the player meant to write.
                this.fill_leagues(&[], None);
                this.set_status(err, true);
                save.set_sensitive(false);
                this.login_button.set_sensitive(false);
            }
        }
        this.load_leagues(league_status);
        choose.connect_clicked({
            let this = this.clone();
            move |_| this.choose_client_txt()
        });
        if let Ok(file) = file {
            let this = this.clone();
            this.login_button.connect_clicked({
                let (this, file) = (this.clone(), file.clone());
                move |_| this.toggle_login(&file)
            });
            save.connect_clicked(move |_| this.save(&file));
        }
        this.window.present();
    }

    /// Asks poe.ninja for its leagues on a thread; the list replaces the
    /// dropdown's, keeping the choice.
    fn load_leagues(&self, status: gtk::Label) {
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(PriceSource::new(Http::new(), None).leagues(Instant::now()));
        });
        let this = self.clone();
        glib::spawn_future_local(async move {
            match rx.recv().await {
                Ok(Ok(leagues)) => {
                    let chosen = this.chosen_league();
                    this.fill_leagues(&leagues, chosen.as_deref());
                }
                Ok(Err(err)) => {
                    status.set_label(&format!("Couldn't list the leagues: {err}"));
                    status.set_css_classes(&["error"]);
                }
                Err(_) => {}
            }
        });
    }

    /// Fills the dropdown: poe.ninja's first league, `leagues`, and `chosen`
    /// if poe.ninja doesn't list it; selects `chosen`.
    fn fill_leagues(&self, leagues: &[Arc<str>], chosen: Option<&str>) {
        let mut entries: Vec<Option<Arc<str>>> = Vec::with_capacity(leagues.len() + 2);
        entries.push(None);
        entries.extend(leagues.iter().cloned().map(Some));
        if let Some(chosen) = chosen
            && !leagues.iter().any(|league| &**league == chosen)
        {
            entries.push(Some(chosen.into()));
        }
        let first = match leagues.first() {
            Some(first) => format!("Current challenge league ({first})"),
            None => "Current challenge league".to_owned(),
        };
        let names: Vec<&str> = entries.iter().map(|entry| entry.as_deref().unwrap_or(&first)).collect();
        self.league_names.splice(0, self.league_names.n_items(), &names);
        let selected = entries.iter().position(|entry| entry.as_deref() == chosen).unwrap_or(0);
        self.league.set_selected(selected as u32);
        *self.leagues.borrow_mut() = entries;
    }

    fn chosen_league(&self) -> Option<Arc<str>> {
        self.leagues.borrow().get(self.league.selected() as usize).cloned().flatten()
    }

    fn choose_client_txt(&self) {
        let dialog = gtk::FileDialog::builder().title("Choose Client.txt").modal(true).build();
        let typed = PathBuf::from(self.client_txt.text().trim());
        if let Some(folder) = typed.parent().filter(|folder| folder.is_dir()) {
            dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
        }
        let entry = self.client_txt.clone();
        dialog.open(Some(&self.window), gio::Cancellable::NONE, move |chosen| {
            if let Some(path) = chosen.ok().and_then(|file| file.path()) {
                entry.set_text(&path.to_string_lossy());
            }
        });
    }

    /// Checks the Client.txt path, then writes both settings into `file`.
    fn save(&self, file: &Path) {
        let typed = self.client_txt.text();
        let client_txt = match typed.trim() {
            "" => None,
            typed => Some(Path::new(typed)),
        };
        if let Some(Err(problem)) = client_txt.map(client_log::check_path) {
            self.path_problem.set_label(&problem);
            self.path_problem.set_visible(true);
            self.set_status("Not saved: Client.txt can't be used.", true);
            return;
        }
        self.path_problem.set_visible(false);
        let league = self.chosen_league();
        let written = change(file, |settings| {
            settings.set_league(league.as_deref());
            settings.set_client_txt(client_txt);
        });
        match written {
            Ok(()) => self.set_status("Saved. A running overlay uses the new settings within a second.", false),
            Err(err) => self.set_status(&err, true),
        }
    }

    fn set_status(&self, text: &str, error: bool) {
        self.status.set_label(text);
        self.status.set_css_classes(if error { &["error"] } else { &["dim-label"] });
    }

    /// Logs in or out: written into `file` at once.
    fn toggle_login(&self, file: &Path) {
        let on = !self.logged_in.get();
        match change(file, |settings| settings.set_login(on)) {
            Ok(()) => self.show_login(on, true),
            Err(err) => self.set_login_status(&err, true),
        }
    }

    /// Shows whether the player is logged in; when they are, looks for the
    /// login in their browsers every few seconds until found. After a
    /// click on Log in, opens the login page if no browser holds one.
    fn show_login(&self, on: bool, clicked: bool) {
        self.logged_in.set(on);
        self.login_shown.set(self.login_shown.get() + 1);
        self.login_button.set_label(if on { "Log out" } else { "Log in" });
        if !on {
            let text = if clicked { "Logged out. Your browser stays logged in on pathofexile.com." } else { "Not logged in." };
            self.set_login_status(text, false);
            return;
        }
        self.set_login_status("Looking for the login in your browsers…", false);
        let (this, shown) = (self.clone(), self.login_shown.get());
        glib::spawn_future_local(async move {
            let mut opened = !clicked;
            loop {
                let read = gio::spawn_blocking(|| {
                    let (found, problems) = login::from_browsers();
                    (Login::choose(found, SystemTime::now()).map(|login| login.browser), problems)
                })
                .await;
                let Ok((found, problems)) = read else { return };
                if this.login_shown.get() != shown {
                    return;
                }
                if let Some(browser) = found {
                    this.set_login_status(&format!("Logged in with {browser}."), false);
                    return;
                }
                let mut text = String::from("Log in on pathofexile.com in your browser; this notices it by itself.");
                if !opened {
                    opened = true;
                    if let Err(err) = gio::AppInfo::launch_default_for_uri(login::LOGIN_PAGE, gio::AppLaunchContext::NONE) {
                        text = format!("Couldn't open your browser ({err}): log in on {} there; this notices it by itself.", login::LOGIN_PAGE);
                    }
                }
                for problem in &problems {
                    text.push('\n');
                    text.push_str(problem);
                }
                this.set_login_status(&text, !problems.is_empty());
                glib::timeout_future(Duration::from_secs(3)).await;
            }
        });
    }

    fn set_login_status(&self, text: &str, error: bool) {
        self.login_status.set_label(text);
        self.login_status.set_css_classes(if error { &["error"] } else { &["dim-label"] });
    }
}

/// The settings file as it is on disk now; refused if it has a mistake (one
/// the overlay would reject too), since a save then wouldn't take effect.
fn read(file: &Path) -> Result<ConfigFile, String> {
    let text = config::read_text(file)?;
    let mistake = |err: String| format!("{} has a mistake; fix it in a text editor first. {err}", file.display());
    Config::parse(&text).map_err(mistake)?;
    ConfigFile::parse(&text).map_err(mistake)
}

/// Changes the settings in the file as it is now, so changes made since
/// the window opened are kept.
fn change(file: &Path, edit: impl FnOnce(&mut ConfigFile)) -> Result<(), String> {
    let mut settings = read(file)?;
    edit(&mut settings);
    files::write_atomic(file, &settings.to_string()).map_err(|err| format!("Couldn't save {}: {err}", file.display()))
}

/// What leaving the Client.txt path empty means on this machine.
fn steam_hint() -> String {
    match poe1::client_log_path(None, &steam::default_roots()) {
        Some(found) => format!("Leave empty to use Steam's: {}", found.display()),
        None => "Steam doesn't have the game installed: choose Client.txt in the game's logs folder (Lutris, Heroic…).".to_owned(),
    }
}

fn label(text: &str) -> gtk::Label {
    gtk::Label::builder().label(text).xalign(0.0).build()
}

fn hint(text: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).wrap(true).build();
    label.add_css_class("dim-label");
    label
}
