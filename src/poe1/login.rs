//! Logging in to the trade site through the player's browser: after they
//! log in on pathofexile.com there, the browser keeps the login in
//! cookies, which searches then carry. Only these cookies are kept:
//! the session, the trade token and Cloudflare's clearance.

use std::time::{Duration, Instant, SystemTime};

use crate::browsers::{self, Cookie};
use crate::keyring::SystemKeyring;

/// The site whose cookies are read.
pub const DOMAIN: &str = "pathofexile.com";
pub const LOGIN_PAGE: &str = "https://www.pathofexile.com/login";

/// The login session (30 days).
const SESSION: &str = "POESESSID";
/// Sent in this order; the trade token lasts a day, Cloudflare's clearance
/// a year (and holds only for the browser's User-Agent).
const KEPT: [&str; 3] = [SESSION, "POETOKEN", "cf_clearance"];

/// What one browser profile holds for pathofexile.com.
pub struct Found {
    pub browser: &'static str,
    /// The profile belongs to the default browser.
    pub default: bool,
    pub user_agent: Option<String>,
    pub cookies: Vec<Cookie>,
}

/// A login read from a browser. Holds secrets: no `Debug`, never logged.
pub struct Login {
    pub browser: &'static str,
    user_agent: Option<String>,
    cookie_header: String,
    /// When the first of its cookies expires; `None` if all last until the
    /// browser closes.
    expires: Option<SystemTime>,
    /// It has a trade token (the browser may not have one yet).
    has_token: bool,
}

impl Login {
    /// The `Cookie` header's value.
    pub fn cookie_header(&self) -> &str {
        &self.cookie_header
    }

    /// The User-Agent of the browser it came from, when known.
    pub fn user_agent(&self) -> Option<&str> {
        self.user_agent.as_deref()
    }

    /// The login in `found` at `now`: the default browser's if it holds
    /// one, else the newest. Cookies expired at `now` are left out; without
    /// a session there's no login.
    pub fn choose(mut found: Vec<Found>, now: SystemTime) -> Option<Login> {
        let live = |cookie: &&Cookie| cookie.expires.is_none_or(|expires| expires > now);
        let session = |found: &Found| found.cookies.iter().filter(live).filter(|cookie| cookie.name == SESSION).map(|cookie| cookie.created).max();
        let newest = |default: bool| found.iter().enumerate().filter(|(_, found)| found.default == default).filter_map(|(at, found)| Some((session(found)?, at))).max();
        let (_, at) = newest(true).or_else(|| newest(false))?;
        let chosen = found.swap_remove(at);

        let mut cookie_header = String::new();
        let mut expires = None::<SystemTime>;
        let mut has_token = false;
        for name in KEPT {
            let Some(cookie) = chosen.cookies.iter().filter(live).filter(|cookie| cookie.name == name).max_by_key(|cookie| cookie.created) else { continue };
            if !cookie_header.is_empty() {
                cookie_header.push_str("; ");
            }
            cookie_header.push_str(name);
            cookie_header.push('=');
            cookie_header.push_str(&cookie.value);
            if let Some(at) = cookie.expires {
                expires = Some(expires.map_or(at, |first| first.min(at)));
            }
            has_token |= name == KEPT[1];
        }
        Some(Login { browser: chosen.browser, user_agent: chosen.user_agent, cookie_header, expires, has_token })
    }
}

/// What the installed browsers hold for pathofexile.com, and why profiles
/// couldn't be read (one line each).
pub fn from_browsers() -> (Vec<Found>, Vec<String>) {
    let (mut found, mut problems) = (Vec::new(), Vec::new());
    let Some(home) = std::env::var_os("HOME") else { return (found, problems) };
    let default = browsers::default_browser();
    for profile in browsers::profiles(home.as_ref(), default.as_deref()) {
        match profile.cookies(DOMAIN, &SystemKeyring) {
            Ok(cookies) => found.push(Found { browser: profile.browser.name, default: profile.browser.is(default.as_deref()), user_agent: profile.user_agent, cookies }),
            Err(err) => problems.push(err.to_string()),
        }
    }
    (found, problems)
}

/// Reads what the player's browsers hold for pathofexile.com.
pub type ReadBrowsers = Box<dyn FnMut() -> Vec<Found> + Send>;

/// How often browsers are read again while they hold no login, or one
/// without a trade token: the player may be logging in right now.
const READ_AGAIN: Duration = Duration::from_secs(60);

/// The login the browsers hold, read again when needed.
pub struct BrowserLogin {
    read: ReadBrowsers,
    /// The login and when its first cookie expires.
    held: Option<(Login, Option<Instant>)>,
    read_at: Option<Instant>,
}

impl BrowserLogin {
    pub fn new(read: ReadBrowsers) -> BrowserLogin {
        BrowserLogin { read, held: None, read_at: None }
    }

    /// The login to send at `now`. The browsers are read again once one of
    /// its cookies expires (the trade token, after a day: the browser has a
    /// new one if the player opened the site since), and at most once a
    /// minute while there's no login or it has no trade token.
    pub fn current(&mut self, now: Instant) -> Option<&Login> {
        let expired = |(_, until): &(Login, Option<Instant>)| until.is_some_and(|until| now >= until);
        let due = self.read_at.is_none_or(|at| now.duration_since(at) >= READ_AGAIN);
        let read = match &self.held {
            None => due,
            Some(held) => expired(held) || (!held.0.has_token && due),
        };
        if read {
            let wall = SystemTime::now();
            self.held = Login::choose((self.read)(), wall).map(|login| {
                let until = login.expires.map(|at| now + at.duration_since(wall).unwrap_or_default());
                (login, until)
            });
            self.read_at = Some(now);
        }
        self.held.as_ref().map(|(login, _)| login)
    }
}
