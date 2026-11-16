//! Searching the trade site from the overlay: the cheapest listings of items
//! like the one checked, within the rate limits the site states.
//!
//! The trade endpoints aren't in GGG's API reference; tools like Awakened PoE
//! Trade use them the same way.

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::prices::encode;
use super::trade::{ListingKind, TRADE_SITE, TradeQuery};

/// Listings shown: one fetch request takes at most 10.
const SHOWN: usize = 10;
/// The same search within this long is answered from the last one.
const REUSE_FOR: Duration = Duration::from_secs(300);
/// After a 403, how long no search is sent: repeated refusals count against
/// the player's address on GGG's side.
const REFUSED_FOR: Duration = Duration::from_secs(300);

/// Sends requests to the trade site.
pub trait TradeHttp {
    /// POSTs `body` as JSON, or GETs without one. Any HTTP status is a
    /// response; `Err` is for no answer at all.
    fn send(&self, url: &str, body: Option<&str>) -> Result<TradeResponse, String>;
}

#[derive(Debug, Clone)]
pub struct TradeResponse {
    pub status: u16,
    /// At least the rate limit headers and `Retry-After`.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl TradeResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }
}

/// What a search found: how many listings match, and the cheapest.
#[derive(Debug, PartialEq)]
pub struct Listings {
    pub total: u32,
    pub cheapest: Vec<Listing>,
}

#[derive(Debug, PartialEq)]
pub struct Listing {
    /// Amount and currency (`chaos`, `divine`…); `None` if not priced.
    pub price: Option<(f64, String)>,
    /// When it was listed, in seconds since 1970.
    pub listed: Option<i64>,
    pub kind: ListingKind,
    /// An in-person seller is online but away.
    pub afk: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeError {
    /// A rate limit is reached; searching works again after `wait`.
    Limited { wait: Duration },
    /// The site answered 403 (Cloudflare or GGG); not asked again for `wait`.
    Refused { wait: Duration },
    Failed(String),
}

impl From<String> for TradeError {
    fn from(message: String) -> TradeError {
        TradeError::Failed(message)
    }
}

impl fmt::Display for TradeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TradeError::Limited { wait } => write!(f, "The trade site's rate limit is reached; you can search again in {}.", duration(*wait)),
            TradeError::Refused { wait } => write!(
                f,
                "The trade site refused the search (HTTP 403); you can search again in {}. Open it on the trade site meanwhile.",
                duration(*wait)
            ),
            TradeError::Failed(message) => f.write_str(message),
        }
    }
}

/// `42 s` under a minute and a half, else `3 min`; rounded up, ignoring
/// less than a millisecond (the time the answer took).
fn duration(wait: Duration) -> String {
    let seconds = (wait.as_millis() as u64).div_ceil(1000);
    if seconds < 90 { format!("{seconds} s") } else { format!("{} min", seconds.div_ceil(60)) }
}

/// How long ago something was listed: `just now`, `2 min`, `3 h`, `2 d`.
pub fn age(seconds: i64) -> String {
    match seconds {
        ..60 => "just now".to_owned(),
        60..3600 => format!("{} min", seconds / 60),
        3600..86_400 => format!("{} h", seconds / 3600),
        _ => format!("{} d", seconds / 86_400),
    }
}

/// Searches the trade site, one endpoint's rate limits each for searches
/// and fetches.
pub struct TradeSource<H> {
    http: H,
    search_limits: RateLimits,
    fetch_limits: RateLimits,
    refused_until: Option<Instant>,
    /// The last search (league and query), what the site answered, when.
    searched: Option<(String, Instant, Search)>,
    /// The listings fetched for it.
    found: Option<Arc<Listings>>,
}

/// A search answer: the listings' ids, cheapest first.
#[derive(Deserialize)]
struct Search {
    id: String,
    result: Vec<String>,
    total: u32,
}

impl<H: TradeHttp> TradeSource<H> {
    pub fn new(http: H) -> Self {
        Self { http, search_limits: RateLimits::default(), fetch_limits: RateLimits::default(), refused_until: None, searched: None, found: None }
    }

    /// The cheapest online listings like `query` in `league`. The same search
    /// within 5 minutes reuses the last answer, also when only its fetch was
    /// held back by a rate limit.
    pub fn listings(&mut self, query: &TradeQuery, league: &str, now: Instant) -> Result<Arc<Listings>, TradeError> {
        let body = query.to_json();
        let key = format!("{league}\n{body}");
        let reusable = self.searched.as_ref().is_some_and(|(last, at, _)| *last == key && now.duration_since(*at) < REUSE_FOR);
        if !reusable {
            if let Some(until) = self.refused_until.filter(|until| *until > now) {
                return Err(TradeError::Refused { wait: until - now });
            }
            self.found = None;
            let url = format!("{TRADE_SITE}/api/trade/search/{}", encode(league));
            let search = self.request(Endpoint::Search, &url, Some(&body), now)?;
            self.searched = Some((key, now, search));
        } else if let Some(found) = &self.found {
            return Ok(found.clone());
        }
        let (_, _, search) = self.searched.as_ref().expect("searched above");
        let (total, ids, query_id) = (search.total, search.result[..search.result.len().min(SHOWN)].join(","), search.id.clone());
        let cheapest = if ids.is_empty() {
            Vec::new()
        } else {
            let url = format!("{TRADE_SITE}/api/trade/fetch/{ids}?query={query_id}");
            let fetched: Fetched = self.request(Endpoint::Fetch, &url, None, now)?;
            fetched.result.into_iter().flatten().map(Listing::from).collect()
        };
        let found = Arc::new(Listings { total, cheapest });
        self.found = Some(found.clone());
        Ok(found)
    }

    /// Sends one request within `endpoint`'s limits and reads its answer.
    fn request<T: for<'de> Deserialize<'de>>(&mut self, endpoint: Endpoint, url: &str, body: Option<&str>, now: Instant) -> Result<T, TradeError> {
        let limits = match endpoint {
            Endpoint::Search => &mut self.search_limits,
            Endpoint::Fetch => &mut self.fetch_limits,
        };
        if let Some(until) = limits.wait_until(now) {
            return Err(TradeError::Limited { wait: until - now });
        }
        limits.sent(now);
        let asked = Instant::now();
        let response = self.http.send(url, body).map_err(|err| TradeError::Failed(format!("Couldn't reach the trade site: {err}")))?;
        // Waits the site states count from its answer, which may take seconds.
        let answered = now + asked.elapsed();
        limits.update(|name| response.header(name), answered);
        match response.status {
            200 => serde_json::from_str(&response.body).map_err(|err| TradeError::Failed(format!("The trade site sent an unexpected answer ({err})."))),
            403 => {
                self.refused_until = Some(answered + REFUSED_FOR);
                Err(TradeError::Refused { wait: REFUSED_FOR })
            }
            429 => {
                // Without a Retry-After or restriction stated, wait a minute.
                if limits.wait_until(answered).is_none() {
                    limits.block_until(answered + Duration::from_secs(60));
                }
                let until = limits.wait_until(answered).unwrap_or(answered);
                Err(TradeError::Limited { wait: until - answered })
            }
            status => {
                #[derive(Deserialize)]
                struct Failure {
                    error: Message,
                }
                #[derive(Deserialize)]
                struct Message {
                    message: String,
                }
                let message = serde_json::from_str::<Failure>(&response.body).map_or_else(|_| format!("HTTP {status}"), |failure| failure.error.message);
                Err(TradeError::Failed(format!("The trade site didn't search: {message}.")))
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Endpoint {
    Search,
    Fetch,
}

/// A fetch answer; an entry is `null` for a listing removed since the search.
#[derive(Deserialize)]
struct Fetched {
    result: Vec<Option<FetchedItem>>,
}

#[derive(Deserialize)]
struct FetchedItem {
    listing: FetchedListing,
}

#[derive(Deserialize)]
struct FetchedListing {
    indexed: Option<String>,
    price: Option<FetchedPrice>,
    account: FetchedAccount,
    /// Gold the buyer pays on an instant buyout.
    fee: Option<f64>,
}

#[derive(Deserialize)]
struct FetchedPrice {
    amount: f64,
    currency: String,
}

#[derive(Deserialize)]
struct FetchedAccount {
    online: Option<Online>,
}

#[derive(Deserialize)]
struct Online {
    status: Option<String>,
}

impl From<FetchedItem> for Listing {
    fn from(item: FetchedItem) -> Listing {
        let listing = item.listing;
        // A Merchant's Tab listing carries the gold fee the buyer pays.
        let kind = if listing.fee.is_some() { ListingKind::InstantBuyout } else { ListingKind::InPerson };
        let away = listing.account.online.and_then(|online| online.status).is_some_and(|status| status == "afk");
        Listing {
            price: listing.price.map(|price| (price.amount, price.currency)),
            listed: listing.indexed.as_deref().and_then(unix_seconds),
            kind,
            // An instant buyout needs no one at the keyboard.
            afk: kind == ListingKind::InPerson && away,
        }
    }
}

/// `2026-11-15T11:25:14Z` in seconds since 1970 (UTC).
fn unix_seconds(time: &str) -> Option<i64> {
    let number = |range: std::ops::Range<usize>| time.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    // Days since 1970 of a civil date (Howard Hinnant's days_from_civil).
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
}

/// One endpoint's rate limits, as the trade site's response headers state
/// them (`X-Rate-Limit-Ip: 5:10:60,15:60:300`: at most 5 requests in 10
/// seconds…), plus the requests sent since.
#[derive(Debug, Default)]
pub struct RateLimits {
    /// At most `.0` requests in `.1`.
    rules: Vec<(u32, Duration)>,
    sent: VecDeque<Instant>,
    /// A restriction the site imposed (or a `Retry-After`).
    blocked_until: Option<Instant>,
}

impl RateLimits {
    /// When the next request may go out; `None`: now.
    pub fn wait_until(&self, now: Instant) -> Option<Instant> {
        let windows = self.rules.iter().filter_map(|&(max, period)| self.window_full_until(max, period, now));
        windows.chain(self.blocked_until).filter(|until| *until > now).max()
    }

    pub fn sent(&mut self, now: Instant) {
        self.sent.push_back(now);
        // Until the first answer states the rules, keep a day's worth.
        let longest = self.rules.iter().map(|(_, period)| *period).max().unwrap_or(Duration::from_secs(86_400));
        while self.sent.front().is_some_and(|first| now.duration_since(*first) >= longest) {
            self.sent.pop_front();
        }
    }

    /// Takes the limits and their state from a response's headers
    /// (`header` looks one up by name).
    pub fn update<'a>(&mut self, header: impl Fn(&str) -> Option<&'a str>, now: Instant) {
        if let Some(seconds) = header("retry-after").and_then(|value| value.trim().parse().ok()) {
            self.block_until(now + seconds_capped(seconds));
        }
        let Some(names) = header("x-rate-limit-rules") else { return };
        let mut rules = Vec::new();
        let mut full = Vec::new();
        for name in names.split(',').map(str::trim).filter(|name| !name.is_empty()) {
            let triples = |suffix: &str| header(&format!("x-rate-limit-{name}{suffix}")).map(triples).unwrap_or_default();
            let limits = triples("");
            for (hits, period, restricted) in triples("-state") {
                if restricted > 0 {
                    self.block_until(now + seconds_capped(restricted));
                }
                if let Some(&(max, ..)) = limits.iter().find(|(_, rule_period, _)| *rule_period == period)
                    && hits >= max
                {
                    full.push((hits, seconds_capped(period)));
                }
            }
            rules.extend(limits.into_iter().map(|(max, period, _)| (u32::try_from(max).unwrap_or(u32::MAX), seconds_capped(period))));
        }
        self.rules = rules;
        // A rule full of requests this program didn't send (another tool on
        // the same address) can't be timed from them: wait it out whole.
        for (hits, period) in full {
            if (self.in_window(period, now) as u64) < hits {
                self.block_until(now + period);
            }
        }
    }

    /// Sends nothing before `until`.
    pub(crate) fn block_until(&mut self, until: Instant) {
        self.blocked_until = Some(self.blocked_until.map_or(until, |known| known.max(until)));
    }

    fn in_window(&self, period: Duration, now: Instant) -> usize {
        self.sent.iter().filter(|sent| now.duration_since(**sent) < period).count()
    }

    /// When the oldest request in a full window leaves it.
    fn window_full_until(&self, max: u32, period: Duration, now: Instant) -> Option<Instant> {
        let in_window = self.in_window(period, now);
        let max = max as usize;
        let oldest = self.sent.iter().filter(|sent| now.duration_since(**sent) < period).nth(in_window.checked_sub(max)?)?;
        Some(*oldest + period)
    }
}

/// A wait the site states, at most a day: a garbled header mustn't overflow
/// the clock or block for ever.
fn seconds_capped(seconds: u64) -> Duration {
    Duration::from_secs(seconds.min(86_400))
}

/// `5:10:60,15:60:300` → (5, 10, 60), (15, 60, 300).
fn triples(value: &str) -> Vec<(u64, u64, u64)> {
    value
        .split(',')
        .filter_map(|triple| {
            let mut numbers = triple.trim().splitn(3, ':').map(|number| number.parse().ok());
            Some((numbers.next()??, numbers.next()??, numbers.next()??))
        })
        .collect()
}
