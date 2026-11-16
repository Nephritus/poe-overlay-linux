//! Reading one site's cookies from a browser profile's cookie file. The
//! files are built here with the browsers' own tables: Firefox's
//! `moz_cookies` (schema 17, from a Firefox 159 profile) and Chromium's
//! `cookies` (schema 24, from Chrome 156), copied on 2026-11-16. Chromium's
//! encrypted values were made with `openssl enc -aes-128-cbc` and keys from
//! `openssl kdf … PBKDF2`, not with the code under test.

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use poe_overlay::browsers::{Browser, Cookie, Keyring, KeyringError, Profile};
use rusqlite::Connection;

const MOZ_COOKIES: &str = "CREATE TABLE moz_cookies (id INTEGER PRIMARY KEY, originAttributes TEXT NOT NULL DEFAULT '', name TEXT, value TEXT, host TEXT, path TEXT, expiry INTEGER, lastAccessed INTEGER, creationTime INTEGER, isSecure INTEGER, isHttpOnly INTEGER, inBrowserElement INTEGER DEFAULT 0, sameSite INTEGER DEFAULT 0, schemeMap INTEGER DEFAULT 0, isPartitionedAttributeSet INTEGER DEFAULT 0, updateTime INTEGER, CONSTRAINT moz_uniqueid UNIQUE (name, host, path, originAttributes))";
const CHROMIUM_COOKIES: &str = "CREATE TABLE cookies(creation_utc INTEGER NOT NULL,host_key TEXT NOT NULL,top_frame_site_key TEXT NOT NULL,name TEXT NOT NULL,value TEXT NOT NULL,encrypted_value BLOB NOT NULL,path TEXT NOT NULL,expires_utc INTEGER NOT NULL,is_secure INTEGER NOT NULL,is_httponly INTEGER NOT NULL,last_access_utc INTEGER NOT NULL,has_expires INTEGER NOT NULL,is_persistent INTEGER NOT NULL,priority INTEGER NOT NULL,samesite INTEGER NOT NULL,source_scheme INTEGER NOT NULL,source_port INTEGER NOT NULL,last_update_utc INTEGER NOT NULL,source_type INTEGER NOT NULL,has_cross_site_ancestor INTEGER NOT NULL); CREATE TABLE meta(key LONGVARCHAR NOT NULL UNIQUE PRIMARY KEY, value LONGVARCHAR);";

/// 2026-12-16 11:38:28 UTC, and when the cookie was made, a month before.
const EXPIRES: u64 = 1_797_421_108;
const CREATED: u64 = 1_794_829_108;
/// Seconds from 1601 (Chromium's epoch) to 1970.
const FROM_1601: u64 = 11_644_473_600;

/// `v10`: the fixed key (password "peanuts"); SHA-256 of the host first.
const SESSION_V10: &str = "763130 01d714c509177e26e6242e9daaf24c29c13c7befab4344dc350a438bce5c763c31beef74103270edf75e39825b8ed124";
/// `v11`: keyring password "test keyring password", host www.pathofexile.com.
const TOKEN_V11: &str = "763131 e650090f1a0751cc511e28e6b948cffdf42fa7a2c4e84d19c8ca41a7f04cb39eff4d39c9f4f5ea7e9deb1792fcbc8eff";
/// `v11` with an empty password: Chromium's key when the keyring gave none.
const CLEARANCE_V11_EMPTY: &str = "763131 a918d7bed5adde1ed820c747a291ad07f96d4001aa27dcadb886d7b184b2d3e5869322175cbec56aac764ecea0eda7e8";
/// `v10` "old" without the host's hash (files before schema 24).
const OLD_V10: &str = "763130 f16df58b19192ff350b87b80f50eb0db";

fn hex(text: &str) -> Vec<u8> {
    let text = text.replace(' ', "");
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
}

/// Answers for Chrome's key; Firefox never asks.
struct FakeKeyring(fn() -> Result<Vec<u8>, KeyringError>);

impl Keyring for FakeKeyring {
    fn password(&self, name: &str) -> Result<Vec<u8>, KeyringError> {
        assert_eq!(name, "Chrome");
        (self.0)()
    }
}

const NO_KEYRING: FakeKeyring = FakeKeyring(|| panic!("keyring asked"));
const TEST_KEYRING: FakeKeyring = FakeKeyring(|| Ok(b"test keyring password".to_vec()));

fn firefox(dir: &Path) -> Profile {
    Profile { browser: Browser::named("Firefox").unwrap(), dir: dir.to_owned(), user_agent: None }
}

fn chrome(dir: &Path, version: u32) -> (Profile, Connection) {
    let db = Connection::open(dir.join("Cookies")).unwrap();
    db.execute_batch(CHROMIUM_COOKIES).unwrap();
    db.execute("INSERT INTO meta VALUES ('version', ?1)", [version.to_string()]).unwrap();
    (Profile { browser: Browser::named("Chrome").unwrap(), dir: dir.to_owned(), user_agent: None }, db)
}

fn add_chromium(db: &Connection, host: &str, name: &str, value: &str, encrypted: &str) {
    let (created, expires) = (((CREATED + FROM_1601) * 1_000_000) as i64, ((EXPIRES + FROM_1601) * 1_000_000) as i64);
    db.execute(
        "INSERT INTO cookies VALUES (?1, ?2, '', ?3, ?4, ?5, '/', ?6, 1, 1, ?1, 1, 1, 1, 0, 2, 443, ?1, 0, 0)",
        (created, host, name, value, hex(encrypted), expires),
    )
    .unwrap();
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

    let cookies = firefox(dir.path()).cookies("pathofexile.com", &NO_KEYRING).unwrap();
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

    let cookies = firefox(dir.path()).cookies("pathofexile.com", &NO_KEYRING).unwrap();
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

    let cookies = firefox(dir.path()).cookies("pathofexile.com", &NO_KEYRING).unwrap();
    assert_eq!(names(&cookies), [("POESESSID", "session")]);
    drop(db);
}

#[test]
fn a_profile_without_a_cookie_file_is_an_error_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let Err(err) = firefox(dir.path()).cookies("pathofexile.com", &NO_KEYRING) else { panic!("read a missing file") };
    assert!(err.to_string().contains("cookies.sqlite"), "{err}");
}

#[test]
fn chromium_cookies_are_decrypted_with_the_fixed_key_or_the_keyring_s() {
    let dir = tempfile::tempdir().unwrap();
    let (chrome, db) = chrome(dir.path(), 24);
    add_chromium(&db, ".pathofexile.com", "POESESSID", "", SESSION_V10);
    add_chromium(&db, "www.pathofexile.com", "POETOKEN", "", TOKEN_V11);
    add_chromium(&db, ".pathofexile.com", "cf_clearance", "", CLEARANCE_V11_EMPTY);
    add_chromium(&db, "pathofexile.com", "plain", "not encrypted", "");
    // Never decrypted: it would fail.
    add_chromium(&db, ".example.com", "other", "", "763131 00112233");
    drop(db);

    let cookies = chrome.cookies("pathofexile.com", &TEST_KEYRING).unwrap();
    assert_eq!(names(&cookies), [("POESESSID", "session"), ("POETOKEN", "token"), ("cf_clearance", "clearance"), ("plain", "not encrypted")]);
    assert_eq!(cookies[0].expires, Some(UNIX_EPOCH + Duration::from_secs(EXPIRES)));
    assert_eq!(cookies[0].created, UNIX_EPOCH + Duration::from_secs(CREATED));
}

#[test]
fn older_chromium_files_have_no_host_hash_before_the_value_and_need_no_keyring_for_v10() {
    let dir = tempfile::tempdir().unwrap();
    let (chrome, db) = chrome(dir.path(), 23);
    add_chromium(&db, ".pathofexile.com", "POESESSID", "", OLD_V10);
    drop(db);

    let cookies = chrome.cookies("pathofexile.com", &NO_KEYRING).unwrap();
    assert_eq!(names(&cookies), [("POESESSID", "old")]);
}

#[test]
fn a_key_that_doesnt_decrypt_is_an_error_not_a_garbled_cookie() {
    let dir = tempfile::tempdir().unwrap();
    let (chrome, db) = chrome(dir.path(), 24);
    add_chromium(&db, "www.pathofexile.com", "POETOKEN", "", TOKEN_V11);
    drop(db);

    let Err(err) = chrome.cookies("pathofexile.com", &FakeKeyring(|| Ok(b"another password".to_vec()))) else { panic!("decrypted with the wrong key") };
    assert!(err.to_string().contains("Chrome") && err.to_string().contains("decrypt"), "{err}");
}

#[test]
fn a_locked_or_missing_keyring_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let (chrome, db) = chrome(dir.path(), 24);
    add_chromium(&db, "www.pathofexile.com", "POETOKEN", "", TOKEN_V11);
    drop(db);

    let Err(locked) = chrome.cookies("pathofexile.com", &FakeKeyring(|| Err(KeyringError::Locked))) else { panic!("read without the key") };
    assert!(locked.to_string().contains("locked"), "{locked}");
    let Err(missing) = chrome.cookies("pathofexile.com", &FakeKeyring(|| Err(KeyringError::Missing))) else { panic!("read without the key") };
    assert!(missing.to_string().contains("Chrome Safe Storage"), "{missing}");
}
