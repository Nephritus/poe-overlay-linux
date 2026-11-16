//! Trade searches with the login read from the player's browsers, through a
//! fake trade site and fake browsers.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use poe_overlay::browsers::Cookie;
use poe_overlay::poe1::item::parse;
use poe_overlay::poe1::login::{Found, Login, ReadBrowsers};
use poe_overlay::poe1::trade::{TradeData, TradeQuery};
use poe_overlay::poe1::trade_search::{TradeHttp, TradeResponse, TradeSource};

const HOUR: Duration = Duration::from_secs(3600);
const FIREFOX_159: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:159.0) Gecko/20100101 Firefox/159.0";

/// What a request carried: its Cookie header and User-Agent.
type Carried = Option<(String, Option<String>)>;

/// Answers every search with one listing; logs the login each request
/// carried.
#[derive(Default)]
struct FakeTrade {
    sent: RefCell<Vec<Carried>>,
}

impl TradeHttp for &FakeTrade {
    fn send(&self, url: &str, _body: Option<&str>, login: Option<&Login>) -> Result<TradeResponse, String> {
        self.sent.borrow_mut().push(login.map(|login| (login.cookie_header().to_owned(), login.user_agent().map(str::to_owned))));
        let body = if url.contains("/search/") { r#"{"id":"q","result":["a"],"total":1}"# } else { r#"{"result":[null]}"# };
        Ok(TradeResponse { status: 200, headers: Vec::new(), body: body.to_owned() })
    }
}

fn gloves() -> TradeQuery<'static> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/items/rare-gloves-death-nails.txt")).unwrap();
    TradeQuery::new(&parse(&text).unwrap(), TradeData::bundled()).unwrap()
}

fn cookie(name: &str, value: &str, lasts: Duration, age: Duration) -> Cookie {
    let now = SystemTime::now();
    Cookie { name: name.to_owned(), value: value.to_owned(), expires: Some(now + lasts), created: now - age }
}

fn found(browser: &'static str, default: bool, cookies: Vec<Cookie>) -> Found {
    Found { browser, default, user_agent: Some(format!("{browser} agent")), cookies }
}

type Shared<T> = Arc<Mutex<T>>;

/// Fake browsers that hold what's in the first (changeable), counting their
/// reads in the second.
fn browsers(holds: Vec<Found>) -> (Shared<Vec<Found>>, Shared<u32>, ReadBrowsers) {
    let (holds, reads) = (Arc::new(Mutex::new(holds)), Arc::new(Mutex::new(0)));
    let read = {
        let (holds, reads) = (holds.clone(), reads.clone());
        Box::new(move || {
            *reads.lock().unwrap() += 1;
            std::mem::take(&mut *holds.lock().unwrap())
        })
    };
    (holds, reads, read)
}

fn logged_in_firefox() -> Vec<Found> {
    vec![Found {
        browser: "Firefox",
        default: true,
        user_agent: Some(FIREFOX_159.to_owned()),
        cookies: vec![
            cookie("_ga", "analytics", HOUR, HOUR),
            cookie("POESESSID", "session", 30 * 24 * HOUR, HOUR),
            cookie("cf_clearance", "clearance", 365 * 24 * HOUR, HOUR),
            cookie("POETOKEN", "token", 20 * HOUR, HOUR),
        ],
    }]
}

#[test]
fn searches_carry_the_login_and_the_user_agent_of_its_browser() {
    let trade = FakeTrade::default();
    let mut source = TradeSource::new(&trade);
    let mut held = logged_in_firefox();
    // Firefox keeps a second one per partition or container: the newest wins.
    held[0].cookies.push(cookie("POETOKEN", "older token", 20 * HOUR, 2 * HOUR));
    source.log_in(browsers(held).2);

    source.listings(&gloves(), "Covens", Instant::now()).unwrap();

    let login = Some(("POESESSID=session; POETOKEN=token; cf_clearance=clearance".to_owned(), Some(FIREFOX_159.to_owned())));
    assert_eq!(*trade.sent.borrow(), [login.clone(), login], "search and fetch");
}

#[test]
fn nothing_is_sent_or_read_without_logging_in_or_after_logging_out() {
    let trade = FakeTrade::default();
    let mut source = TradeSource::new(&trade);
    let now = Instant::now();
    source.listings(&gloves(), "Covens", now).unwrap();

    let (_, reads, read) = browsers(logged_in_firefox());
    source.log_in(read);
    source.listings(&gloves(), "Standard", now).unwrap();
    source.log_out();
    source.listings(&gloves(), "Hardcore", now).unwrap();

    let sent: Vec<bool> = trade.sent.borrow().iter().map(Option::is_some).collect();
    assert_eq!(sent, [false, false, true, true, false, false]);
    assert_eq!(*reads.lock().unwrap(), 1);
}

#[test]
fn an_expired_trade_token_is_read_again_from_the_browser() {
    let trade = FakeTrade::default();
    let mut source = TradeSource::new(&trade);
    let (holds, reads, read) = browsers(logged_in_firefox());
    source.log_in(read);
    let now = Instant::now();
    source.listings(&gloves(), "Covens", now).unwrap();

    // The player opened the trade site in the browser, which got a new token.
    let mut renewed = logged_in_firefox();
    renewed[0].cookies[3] = cookie("POETOKEN", "new token", 44 * HOUR, Duration::ZERO);
    *holds.lock().unwrap() = renewed;
    source.listings(&gloves(), "Standard", now + 10 * HOUR).unwrap();
    assert_eq!(*reads.lock().unwrap(), 1, "token still good: browser not read");
    source.listings(&gloves(), "Covens", now + 21 * HOUR).unwrap();

    assert_eq!(*reads.lock().unwrap(), 2);
    let sent = trade.sent.borrow();
    assert_eq!(sent.last().unwrap().as_ref().unwrap().0, "POESESSID=session; POETOKEN=new token; cf_clearance=clearance");
}

#[test]
fn expired_cookies_are_not_sent_and_without_a_session_there_is_no_login() {
    let trade = FakeTrade::default();
    let mut source = TradeSource::new(&trade);
    let mut stale = logged_in_firefox();
    stale[0].cookies[3] = cookie("POETOKEN", "token", Duration::ZERO, 25 * HOUR);
    source.log_in(browsers(stale).2);
    source.listings(&gloves(), "Covens", Instant::now()).unwrap();
    assert_eq!(trade.sent.borrow()[0].as_ref().unwrap().0, "POESESSID=session; cf_clearance=clearance");

    let trade = FakeTrade::default();
    let mut source = TradeSource::new(&trade);
    let mut logged_out = logged_in_firefox();
    logged_out[0].cookies[1] = cookie("POESESSID", "session", Duration::ZERO, 31 * 24 * HOUR);
    source.log_in(browsers(logged_out).2);
    source.listings(&gloves(), "Covens", Instant::now()).unwrap();
    assert_eq!(trade.sent.borrow()[0], None);
}

#[test]
fn browsers_without_a_login_are_read_again_once_a_minute_at_most() {
    let trade = FakeTrade::default();
    let mut source = TradeSource::new(&trade);
    let (holds, reads, read) = browsers(Vec::new());
    source.log_in(read);
    let now = Instant::now();
    source.listings(&gloves(), "Covens", now).unwrap();
    *holds.lock().unwrap() = logged_in_firefox();
    source.listings(&gloves(), "Standard", now + Duration::from_secs(30)).unwrap();
    assert_eq!((*reads.lock().unwrap(), trade.sent.borrow()[2].is_some()), (1, false));

    source.listings(&gloves(), "Hardcore", now + Duration::from_secs(61)).unwrap();
    assert_eq!((*reads.lock().unwrap(), trade.sent.borrow()[4].is_some()), (2, true));
}

#[test]
fn a_login_without_a_trade_token_picks_one_up_within_a_minute() {
    let trade = FakeTrade::default();
    let mut source = TradeSource::new(&trade);
    let mut tokenless = logged_in_firefox();
    tokenless[0].cookies.remove(3);
    let (holds, reads, read) = browsers(tokenless);
    source.log_in(read);
    let now = Instant::now();
    source.listings(&gloves(), "Covens", now).unwrap();
    *holds.lock().unwrap() = logged_in_firefox();
    source.listings(&gloves(), "Standard", now + Duration::from_secs(61)).unwrap();

    assert_eq!(*reads.lock().unwrap(), 2);
    assert_eq!(trade.sent.borrow()[2].as_ref().unwrap().0, "POESESSID=session; POETOKEN=token; cf_clearance=clearance");
}

#[test]
fn the_default_browser_s_login_wins_then_the_newest() {
    let session = |value, age| vec![cookie("POESESSID", value, HOUR, age)];
    let chosen = |holds: Vec<Found>| {
        let trade = FakeTrade::default();
        let mut source = TradeSource::new(&trade);
        source.log_in(browsers(holds).2);
        source.listings(&gloves(), "Covens", Instant::now()).unwrap();
        trade.sent.borrow()[0].clone().unwrap()
    };

    let default_older = chosen(vec![found("Zen", false, session("zen", HOUR)), found("Firefox", true, session("firefox", 2 * HOUR))]);
    assert_eq!(default_older, ("POESESSID=firefox".to_owned(), Some("Firefox agent".to_owned())));
    let newest = chosen(vec![found("Zen", false, session("zen", 2 * HOUR)), found("Floorp", false, session("floorp", HOUR)), found("Firefox", true, Vec::new())]);
    assert_eq!(newest, ("POESESSID=floorp".to_owned(), Some("Floorp agent".to_owned())));
}
