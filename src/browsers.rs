//! Web browsers' profiles and the cookies they keep: which browsers are
//! installed (natively or through Flatpak), what User-Agent each sends, and
//! one site's cookies from a profile's cookie file, read without touching
//! any other site's.

use std::fmt;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes::cipher::{BlockModeDecrypt, KeyIvInit, block_padding::Pkcs7};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};

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
    /// Chrome and its kin: `Cookies`, encrypted with a key kept in the
    /// system keyring as "`keyring` Safe Storage".
    Chromium { keyring: &'static str, agent: Agent },
}

/// The User-Agent a Chromium browser sends.
enum Agent {
    Chrome,
    /// Chrome's plus ` Edg/<version>`.
    Edge,
    /// Its version file doesn't tell the Chromium version (Opera, Vivaldi).
    Unknown,
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
    Browser {
        name: "Chrome",
        family: Family::Chromium { keyring: "Chrome", agent: Agent::Chrome },
        desktop_ids: &["google-chrome.desktop", "com.google.Chrome.desktop"],
        roots: &[".config/google-chrome", ".var/app/com.google.Chrome/config/google-chrome"],
    },
    Browser {
        name: "Chromium",
        family: Family::Chromium { keyring: "Chromium", agent: Agent::Chrome },
        desktop_ids: &["chromium.desktop", "chromium-browser.desktop", "org.chromium.Chromium.desktop", "chromium_chromium.desktop"],
        roots: &[".config/chromium", "snap/chromium/common/chromium", ".var/app/org.chromium.Chromium/config/chromium"],
    },
    Browser {
        name: "Brave",
        family: Family::Chromium { keyring: "Brave", agent: Agent::Chrome },
        desktop_ids: &["brave-browser.desktop", "com.brave.Browser.desktop"],
        roots: &[".config/BraveSoftware/Brave-Browser", ".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser"],
    },
    // Edge, Opera and Vivaldi keep their key under Chromium's or Chrome's
    // name on Linux (as yt-dlp reads them).
    Browser {
        name: "Edge",
        family: Family::Chromium { keyring: "Chromium", agent: Agent::Edge },
        desktop_ids: &["microsoft-edge.desktop", "com.microsoft.Edge.desktop"],
        roots: &[".config/microsoft-edge", ".var/app/com.microsoft.Edge/config/microsoft-edge"],
    },
    Browser {
        name: "Vivaldi",
        family: Family::Chromium { keyring: "Chrome", agent: Agent::Unknown },
        desktop_ids: &["vivaldi-stable.desktop", "com.vivaldi.Vivaldi.desktop"],
        roots: &[".config/vivaldi", ".var/app/com.vivaldi.Vivaldi/config/vivaldi"],
    },
    Browser {
        name: "Opera",
        family: Family::Chromium { keyring: "Chromium", agent: Agent::Unknown },
        desktop_ids: &["opera.desktop", "com.opera.Opera.desktop"],
        roots: &[".config/opera", ".var/app/com.opera.Opera/config/opera"],
    },
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
                if browser.family.cookie_file(&dir).is_file() {
                    let user_agent = match &browser.family {
                        Family::Gecko => gecko_user_agent(&dir),
                        Family::Chromium { agent, .. } => chromium_user_agent(&home.join(root), agent),
                    };
                    found.push(Profile { browser, dir, user_agent });
                }
            }
        }
    }
    found
}

impl Family {
    /// Newer Chromium keeps its cookies under `Network`.
    fn cookie_file(&self, profile: &Path) -> PathBuf {
        match self {
            Family::Gecko => profile.join("cookies.sqlite"),
            Family::Chromium { .. } => Some(profile.join("Network/Cookies")).filter(|file| file.is_file()).unwrap_or_else(|| profile.join("Cookies")),
        }
    }
}

/// Chrome's User-Agent for the version in the browser's `Last Version`
/// file; Chrome reports only its major version.
fn chromium_user_agent(root: &Path, agent: &Agent) -> Option<String> {
    let suffix = match agent {
        Agent::Chrome => "",
        Agent::Edge => " Edg/",
        Agent::Unknown => return None,
    };
    let version = major(std::fs::read_to_string(root.join("Last Version")).ok()?.trim()).filter(|major| *major >= 100)?;
    let mut agent = format!("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{version}.0.0.0 Safari/537.36");
    if !suffix.is_empty() {
        agent.push_str(&format!("{suffix}{version}.0.0.0"));
    }
    Some(agent)
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
    /// A Chromium browser's key couldn't be had from the keyring.
    Keyring { browser: &'static str, keyring: &'static str, err: KeyringError },
    /// The cookies don't decrypt with the key the keyring holds.
    WrongKey { browser: &'static str },
    /// Encrypted in a way this doesn't know (`v12`…).
    UnknownEncryption { browser: &'static str },
}

impl fmt::Display for CookieError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CookieError::Unreadable { file, err } => write!(f, "Couldn't read {}: {err}", file.display()),
            CookieError::Keyring { browser, err: KeyringError::Locked, .. } => {
                write!(f, "{browser}'s cookies need its key from your keyring, which is locked: unlock it, then Log in again.")
            }
            CookieError::Keyring { browser, keyring, err: KeyringError::Missing } => {
                write!(f, "{browser}'s cookies need the key \"{keyring} Safe Storage\", which isn't in your keyring.")
            }
            CookieError::Keyring { browser, err: KeyringError::Unavailable(err), .. } => {
                write!(f, "{browser}'s cookies need its key from your keyring, which can't be reached: {err}")
            }
            CookieError::WrongKey { browser } => write!(f, "{browser}'s cookies can't be decrypted with the key in your keyring."),
            CookieError::UnknownEncryption { browser } => write!(f, "{browser}'s cookies are encrypted in a way the overlay can't read."),
        }
    }
}

/// Where Chromium browsers keep their cookies' key.
pub trait Keyring {
    /// The password stored as "`name` Safe Storage" (`name` is `Chrome`,
    /// `Chromium` or `Brave`).
    fn password(&self, name: &str) -> Result<Vec<u8>, KeyringError>;
}

#[derive(Debug)]
pub enum KeyringError {
    /// It holds the key but is locked; nothing asks to unlock it.
    Locked,
    /// No such key.
    Missing,
    /// No keyring answers, or it failed.
    Unavailable(String),
}

impl Profile {
    /// The cookies of `domain` and its subdomains; other sites' cookies are
    /// never read out of the file. Chromium browsers' are decrypted with
    /// their key from `keyring`, asked only if one needs it.
    pub fn cookies(&self, domain: &str, keyring: &dyn Keyring) -> Result<Vec<Cookie>, CookieError> {
        let file = self.browser.family.cookie_file(&self.dir);
        let unreadable = |err: String| CookieError::Unreadable { file: file.clone(), err };
        match &self.browser.family {
            Family::Gecko => gecko_cookies(&file, domain).map_err(unreadable),
            Family::Chromium { keyring: name, .. } => {
                let rows = chromium_rows(&file, domain).map_err(unreadable)?;
                ChromiumKeys::new(self.browser.name, name, keyring).decrypt(rows)
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

/// Chromium counts microseconds from 1601.
fn since_1601(micros: i64) -> SystemTime {
    const FROM_1601: Duration = Duration::from_secs(11_644_473_600);
    since_1970(Duration::from_micros(micros.max(0) as u64).saturating_sub(FROM_1601))
}

/// One Chromium cookie as stored.
struct ChromiumRow {
    host: String,
    cookie: Cookie,
    encrypted: Vec<u8>,
}

/// Since schema 24 an encrypted value starts with SHA-256 of its host.
struct ChromiumRows {
    hashed: bool,
    rows: Vec<ChromiumRow>,
}

fn chromium_rows(file: &Path, domain: &str) -> Result<ChromiumRows, String> {
    let copy = Copy::of(file).map_err(|err| err.to_string())?;
    let db = copy.open().map_err(|err| err.to_string())?;
    let version: String = db.query_row("SELECT value FROM meta WHERE key = 'version'", [], |row| row.get(0)).map_err(|err| err.to_string())?;
    let mut rows = db
        .prepare("SELECT host_key, name, value, encrypted_value, expires_utc, creation_utc FROM cookies WHERE host_key = ?1 OR substr(host_key, -length(?1) - 1) = '.' || ?1")
        .map_err(|err| err.to_string())?;
    let rows = rows.query_map([domain], |row| {
        // 0 for a cookie that lasts until the browser closes.
        let expires: i64 = row.get(4)?;
        let cookie = Cookie { name: row.get(1)?, value: row.get(2)?, expires: (expires > 0).then(|| since_1601(expires)), created: since_1601(row.get(5)?) };
        Ok(ChromiumRow { host: row.get(0)?, cookie, encrypted: row.get(3)? })
    });
    let rows = rows.and_then(Iterator::collect).map_err(|err| err.to_string())?;
    Ok(ChromiumRows { hashed: version.parse::<u32>().is_ok_and(|version| version >= 24), rows })
}

/// The keys a Chromium browser's cookies may be encrypted with.
struct ChromiumKeys<'k> {
    browser: &'static str,
    name: &'static str,
    keyring: &'k dyn Keyring,
    /// `v11`: the keyring's, then the empty password's (Chromium's when
    /// the keyring gave none); asked once, when first needed.
    v11: Option<(Vec<[u8; 16]>, Option<KeyringError>)>,
}

impl ChromiumKeys<'_> {
    fn new<'k>(browser: &'static str, name: &'static str, keyring: &'k dyn Keyring) -> ChromiumKeys<'k> {
        ChromiumKeys { browser, name, keyring, v11: None }
    }

    fn decrypt(mut self, rows: ChromiumRows) -> Result<Vec<Cookie>, CookieError> {
        rows.rows
            .into_iter()
            .map(|row| {
                let mut cookie = row.cookie;
                if !row.encrypted.is_empty() {
                    cookie.value = self.value(&row.encrypted, &row.host, rows.hashed)?;
                }
                Ok(cookie)
            })
            .collect()
    }

    fn value(&mut self, encrypted: &[u8], host: &str, hashed: bool) -> Result<String, CookieError> {
        let browser = self.browser;
        match encrypted.split_at_checked(3) {
            Some((b"v10", data)) => decrypt(&key(b"peanuts"), data, host, hashed).ok_or(CookieError::WrongKey { browser }),
            Some((b"v11", data)) => {
                let (keys, keyring_error) = self.v11.get_or_insert_with(|| match self.keyring.password(self.name) {
                    Ok(password) => (vec![key(&password), key(b"")], None),
                    Err(err) => (vec![key(b"")], Some(err)),
                });
                if let Some(value) = keys.iter().find_map(|key| decrypt(key, data, host, hashed)) {
                    return Ok(value);
                }
                Err(match keyring_error.take() {
                    Some(err) => CookieError::Keyring { browser, keyring: self.name, err },
                    None => CookieError::WrongKey { browser },
                })
            }
            _ => Err(CookieError::UnknownEncryption { browser }),
        }
    }
}

/// Chromium's key for `password`: PBKDF2-SHA1, one round.
fn key(password: &[u8]) -> [u8; 16] {
    let mut key = [0; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, b"saltysalt", 1, &mut key);
    key
}

/// AES-128-CBC with sixteen spaces as IV; `None` if `key` isn't the one.
fn decrypt(key: &[u8; 16], data: &[u8], host: &str, hashed: bool) -> Option<String> {
    let mut buffer = data.to_vec();
    let plain = cbc::Decryptor::<aes::Aes128>::new(key.into(), &[b' '; 16].into()).decrypt_padded::<Pkcs7>(&mut buffer).ok()?;
    let value = if hashed {
        let (hash, value) = plain.split_at_checked(32)?;
        (hash == Sha256::digest(host.as_bytes()).as_slice()).then_some(value)?
    } else {
        plain
    };
    String::from_utf8(value.to_vec()).ok()
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
