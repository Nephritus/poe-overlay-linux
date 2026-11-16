//! Reading one site's cookies from a browser profile's cookie file. The
//! files are built here with Firefox's own `moz_cookies` table (schema 17,
//! copied from a Firefox 159 profile on 2026-11-16).

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use poe_overlay::browsers::{Browser, Cookie, Profile};
use rusqlite::Connection;

const MOZ_COOKIES: &str = "CREATE TABLE moz_cookies (id INTEGER PRIMARY KEY, originAttributes TEXT NOT NULL DEFAULT '', name TEXT, value TEXT, host TEXT, path TEXT, expiry INTEGER, lastAccessed INTEGER, creationTime INTEGER, isSecure INTEGER, isHttpOnly INTEGER, inBrowserElement INTEGER DEFAULT 0, sameSite INTEGER DEFAULT 0, schemeMap INTEGER DEFAULT 0, isPartitionedAttributeSet INTEGER DEFAULT 0, updateTime INTEGER, CONSTRAINT moz_uniqueid UNIQUE (name, host, path, originAttributes))";

/// 2026-12-16 11:38:28 UTC, and when the cookie was made, a month before.
const EXPIRES: u64 = 1_797_421_108;
const CREATED: u64 = 1_794_829_108;

fn firefox(dir: &Path) -> Profile {
    Profile { browser: Browser::named("Firefox").unwrap(), dir: dir.to_owned(), user_agent: None }
}

fn add(db: &Connection, host: &str, name: &str, value: &str, expiry: u64) {
    db.execute(
        "INSERT INTO moz_cookies (name, value, host, path, expiry, lastAccessed, creationTime, isSecure, isHttpOnly) VALUES (?1, ?2, ?3, '/', ?4, ?5, ?5, 1, 1)",
        (name, value, host, expiry as i64, (CREATED * 1_000_000) as i64),
    )
    .unwrap();
}

fn names(cookies: &[Cookie]) -> Vec<(&str, &str)> {
    let mut names: Vec<_> = cookies.iter().map(|cookie| (cookie.name.as_str(), cookie.value.as_str())).collect();
    names.sort();
    names
}

#[test]
fn only_the_sites_own_cookies_are_read_with_when_they_expire() {
    let dir = tempfile::tempdir().unwrap();
    let db = Connection::open(dir.path().join("cookies.sqlite")).unwrap();
    db.execute(MOZ_COOKIES, ()).unwrap();
    // Firefox 159 writes expiry in milliseconds.
    add(&db, ".pathofexile.com", "POESESSID", "session", EXPIRES * 1000);
    add(&db, "www.pathofexile.com", "POETOKEN", "token", EXPIRES * 1000);
    add(&db, "pathofexile.com", "plain", "host", EXPIRES * 1000);
    add(&db, ".example.com", "POESESSID", "other site", EXPIRES * 1000);
    add(&db, "notpathofexile.com", "lookalike", "other site", EXPIRES * 1000);
    add(&db, "pathofexile.com.evil.net", "suffix", "other site", EXPIRES * 1000);
    drop(db);

    let cookies = firefox(dir.path()).cookies("pathofexile.com").unwrap();
    assert_eq!(names(&cookies), [("POESESSID", "session"), ("POETOKEN", "token"), ("plain", "host")]);
    let session = cookies.iter().find(|cookie| cookie.name == "POESESSID").unwrap();
    assert_eq!(session.expires, Some(UNIX_EPOCH + Duration::from_secs(EXPIRES)));
    assert_eq!(session.created, UNIX_EPOCH + Duration::from_secs(CREATED));
}

#[test]
fn older_firefox_files_with_expiry_in_seconds_are_read_too() {
    let dir = tempfile::tempdir().unwrap();
    let db = Connection::open(dir.path().join("cookies.sqlite")).unwrap();
    db.execute(MOZ_COOKIES, ()).unwrap();
    add(&db, ".pathofexile.com", "POESESSID", "session", EXPIRES);
    drop(db);

    let cookies = firefox(dir.path()).cookies("pathofexile.com").unwrap();
    assert_eq!(cookies[0].expires, Some(UNIX_EPOCH + Duration::from_secs(EXPIRES)));
}

#[test]
fn a_running_browser_s_locked_file_is_read_with_cookies_not_yet_written_back() {
    let dir = tempfile::tempdir().unwrap();
    // Like a running Firefox: write-ahead log, file locked, recent changes
    // still only in the log.
    let db = Connection::open(dir.path().join("cookies.sqlite")).unwrap();
    db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0; PRAGMA locking_mode = EXCLUSIVE;").unwrap();
    db.execute(MOZ_COOKIES, ()).unwrap();
    add(&db, ".pathofexile.com", "POESESSID", "session", EXPIRES * 1000);
    assert!(std::fs::metadata(dir.path().join("cookies.sqlite-wal")).unwrap().len() > 0);

    let cookies = firefox(dir.path()).cookies("pathofexile.com").unwrap();
    assert_eq!(names(&cookies), [("POESESSID", "session")]);
    drop(db);
}

#[test]
fn a_profile_without_a_cookie_file_is_an_error_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let Err(err) = firefox(dir.path()).cookies("pathofexile.com") else { panic!("read a missing file") };
    assert!(err.to_string().contains("cookies.sqlite"), "{err}");
}
