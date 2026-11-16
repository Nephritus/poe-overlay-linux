//! Web browsers' profiles and the cookies they keep: which browsers are
//! installed (natively or through Flatpak), what User-Agent each sends, and
//! one site's cookies from a profile's cookie file, read without touching
//! any other site's.

use std::fmt;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags};

pub struct Browser {
    pub name: &'static str,
    family: Family,
    /// Its desktop entries' IDs, to tell the default browser.
    desktop_ids: &'static [&'static str],
    /// Folders under HOME holding its profiles.
    roots: &'static [&'static str],
}

enum Family {
    /// Firefox and its forks: `cookies.sqlite`, not encrypted.
    Gecko,
}

static BROWSERS: &[Browser] = &[
    Browser {
        name: "Firefox",
        family: Family::Gecko,
        desktop_ids: &["firefox.desktop", "firefox-esr.desktop", "org.mozilla.firefox.desktop", "firefox_firefox.desktop"],
        // Firefox 147 moved from ~/.mozilla to the XDG config folder.
        roots: &[".config/mozilla/firefox", ".mozilla/firefox", "snap/firefox/common/.mozilla/firefox", ".var/app/org.mozilla.firefox/config/mozilla/firefox", ".var/app/org.mozilla.firefox/.mozilla/firefox"],
    },
    Browser {
        name: "LibreWolf",
        family: Family::Gecko,
        desktop_ids: &["librewolf.desktop", "io.gitlab.librewolf-community.desktop"],
        roots: &[".config/librewolf/librewolf", ".librewolf", ".var/app/io.gitlab.librewolf-community/.librewolf"],
    },
    Browser {
        name: "Zen",
        family: Family::Gecko,
        desktop_ids: &["zen.desktop", "zen-browser.desktop", "app.zen_browser.zen.desktop"],
        roots: &[".config/zen", ".zen", ".var/app/app.zen_browser.zen/.zen"],
    },
    Browser { name: "Floorp", family: Family::Gecko, desktop_ids: &["floorp.desktop", "one.ablaze.floorp.desktop"], roots: &[".floorp", ".var/app/one.ablaze.floorp/.floorp"] },
    Browser { name: "Waterfox", family: Family::Gecko, desktop_ids: &["waterfox.desktop", "net.waterfox.waterfox.desktop"], roots: &[".waterfox", ".var/app/net.waterfox.waterfox/.waterfox"] },
];

impl Browser {
    pub fn named(name: &str) -> Option<&'static Browser> {
        BROWSERS.iter().find(|browser| browser.name == name)
    }

    /// Whether the desktop entry `desktop_id` (`firefox.desktop`) is this
    /// browser's.
    pub fn is(&self, desktop_id: Option<&str>) -> bool {
        desktop_id.is_some_and(|id| self.desktop_ids.contains(&id))
    }
}

/// The desktop entry ID of the default browser, the one that opens https
/// links.
pub fn default_browser() -> Option<String> {
    use gtk::gio::prelude::AppInfoExt;
    Some(gtk::gio::AppInfo::default_for_uri_scheme("https")?.id()?.into())
}

/// The profiles of the browsers installed under `home`, the default
/// browser's first: `default` is its desktop entry's ID
/// (`firefox.desktop`). A profile never started (no cookie file) is left out.
pub fn profiles(home: &Path, default: Option<&str>) -> Vec<Profile> {
    let browsers = BROWSERS.iter().filter(|browser| browser.is(default)).chain(BROWSERS.iter().filter(|browser| !browser.is(default)));
    let mut found = Vec::new();
    for browser in browsers {
        for root in browser.roots {
            let Ok(entries) = std::fs::read_dir(home.join(root)) else { continue };
            for dir in entries.flatten().map(|entry| entry.path()) {
                if dir.join("cookies.sqlite").is_file() {
                    let user_agent = gecko_user_agent(&dir);
                    found.push(Profile { browser, dir, user_agent });
                }
            }
        }
    }
    found
}

/// The one set in the profile's preferences, else Firefox's for the Gecko
/// version that last ran the profile. A fork's own version (Zen 1.17)
/// isn't its Gecko version, so it's used only when it looks like one.
fn gecko_user_agent(profile: &Path) -> Option<String> {
    if let Some(set) = std::fs::read_to_string(profile.join("prefs.js")).ok().as_deref().and_then(|prefs| pref(prefs, "general.useragent.override")) {
        return Some(set.to_owned());
    }
    let compatibility = std::fs::read_to_string(profile.join("compatibility.ini")).ok()?;
    let platform = ini_value(&compatibility, "LastPlatformDir").and_then(|dir| std::fs::read_to_string(Path::new(dir).join("platform.ini")).ok());
    let gecko = platform.as_deref().and_then(|platform| major(ini_value(platform, "Milestone")?));
    let version = gecko.or_else(|| major(ini_value(&compatibility, "LastVersion")?).filter(|major| *major >= 100))?;
    Some(format!("Mozilla/5.0 (X11; Linux x86_64; rv:{version}.0) Gecko/20100101 Firefox/{version}.0"))
}

fn ini_value<'t>(text: &'t str, key: &str) -> Option<&'t str> {
    text.lines().find_map(|line| line.strip_prefix(key)?.strip_prefix('=')).map(str::trim)
}

/// A string preference from `prefs.js`: `user_pref("name", "value");`.
fn pref<'t>(prefs: &'t str, name: &str) -> Option<&'t str> {
    prefs.lines().find_map(|line| {
        let rest = line.strip_prefix("user_pref(\"")?.strip_prefix(name)?.strip_prefix("\",")?;
        rest.trim().strip_suffix(");")?.trim().strip_prefix('"')?.strip_suffix('"')
    })
}

fn major(version: &str) -> Option<u32> {
    version.split(['.', '_']).next()?.parse().ok()
}

/// One profile of an installed browser.
pub struct Profile {
    pub browser: &'static Browser,
    pub dir: PathBuf,
    /// The User-Agent the browser sends, when known.
    pub user_agent: Option<String>,
}

pub struct Cookie {
    pub name: String,
    pub value: String,
    /// `None` for one that lasts until the browser closes.
    pub expires: Option<SystemTime>,
    pub created: SystemTime,
}

#[derive(Debug)]
pub enum CookieError {
    Unreadable { file: PathBuf, err: String },
}

impl fmt::Display for CookieError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CookieError::Unreadable { file, err } => write!(f, "Couldn't read {}: {err}", file.display()),
        }
    }
}

impl Profile {
    /// The cookies of `domain` and its subdomains; other sites' cookies are
    /// never read out of the file.
    pub fn cookies(&self, domain: &str) -> Result<Vec<Cookie>, CookieError> {
        match self.browser.family {
            Family::Gecko => {
                let file = self.dir.join("cookies.sqlite");
                gecko_cookies(&file, domain).map_err(|err| CookieError::Unreadable { file, err })
            }
        }
    }
}

fn gecko_cookies(file: &Path, domain: &str) -> Result<Vec<Cookie>, String> {
    let copy = Copy::of(file).map_err(|err| err.to_string())?;
    let db = copy.open().map_err(|err| err.to_string())?;
    let mut rows = db
        .prepare("SELECT name, value, expiry, creationTime FROM moz_cookies WHERE host = ?1 OR substr(host, -length(?1) - 1) = '.' || ?1")
        .map_err(|err| err.to_string())?;
    let cookies = rows.query_map([domain], |row| {
        Ok(Cookie { name: row.get(0)?, value: row.get(1)?, expires: Some(gecko_expiry(row.get(2)?)), created: since_1970(Duration::from_micros(row.get::<_, i64>(3)?.max(0) as u64)) })
    });
    cookies.and_then(Iterator::collect).map_err(|err| err.to_string())
}

/// Firefox 159 writes milliseconds, older versions seconds; no cookie
/// lasts until the year 5138, when seconds would reach the same size.
fn gecko_expiry(raw: i64) -> SystemTime {
    let raw = raw.max(0) as u64;
    since_1970(if raw > 100_000_000_000 { Duration::from_millis(raw) } else { Duration::from_secs(raw) })
}

fn since_1970(time: Duration) -> SystemTime {
    UNIX_EPOCH.checked_add(time).unwrap_or(UNIX_EPOCH)
}

/// A private copy of a cookie file and its write-ahead log, removed when
/// dropped: a running browser keeps its file locked, and its latest
/// changes may still be only in the log.
struct Copy {
    dir: PathBuf,
    file: PathBuf,
}

impl Copy {
    fn of(file: &Path) -> std::io::Result<Copy> {
        static TAKEN: AtomicU32 = AtomicU32::new(0);
        let name = file.file_name().expect("a cookie file has a name");
        let dir = crate::xdg::runtime_dir().join(format!("poe-overlay-cookies.{}.{}", std::process::id(), TAKEN.fetch_add(1, Ordering::Relaxed)));
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        let copy = Copy { file: dir.join(name), dir };
        std::fs::copy(file, &copy.file)?;
        let mut log = name.to_owned();
        log.push("-wal");
        match std::fs::copy(file.with_file_name(&log), copy.dir.join(&log)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        Ok(copy)
    }

    fn open(&self) -> rusqlite::Result<Connection> {
        Connection::open_with_flags(&self.file, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
