//! Prices from poe.ninja's documented PoE 1 economy endpoints, cached so
//! poe.ninja is asked at most once per league per 15 minutes.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Deserialize;

const NINJA: &str = "https://poe.ninja/poe1/api/economy";

/// poe.ninja refreshes PoE 1 prices about every 15 minutes.
const MAX_AGE: Duration = Duration::from_secs(15 * 60);

/// How many currencies the Currency Rates Panel lists.
const LISTED: usize = 12;

/// Fetches a URL's body as text. The seam between the price source and the
/// network: HTTP in the Overlay, recorded responses in tests.
pub trait Fetch {
    fn get(&self, url: &str) -> Result<String, String>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyRate {
    pub name: String,
    pub chaos: f64,
    pub divine: f64,
}

/// The most traded currencies of a league, by trade volume.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyRates {
    pub league: String,
    pub rates: Vec<CurrencyRate>,
}

pub struct PriceSource<F> {
    fetch: F,
    /// From config; `None` means the league poe.ninja lists first.
    league: Option<String>,
    default_league: Option<(Instant, String)>,
    currency: HashMap<String, (Instant, CurrencyRates)>,
}

impl<F: Fetch> PriceSource<F> {
    pub fn new(fetch: F, league: Option<String>) -> Self {
        Self { fetch, league, default_league: None, currency: HashMap::new() }
    }

    /// Currency Rates for the league, from cache if under 15 minutes old.
    /// `now` is the caller's clock. Errors are sentences for the player.
    pub fn currency_rates(&mut self, now: Instant) -> Result<&CurrencyRates, String> {
        let league = self.league(now)?;
        let fresh = self.currency.get(&league).is_some_and(|(at, _)| now.duration_since(*at) < MAX_AGE);
        if !fresh {
            let url = format!("{NINJA}/exchange/current/overview?league={}&type=Currency", encode(&league));
            let body = self.get(&url)?;
            let rates = parse_currency(&body, &league)?;
            self.currency.insert(league.clone(), (now, rates));
        }
        Ok(&self.currency[&league].1)
    }

    fn league(&mut self, now: Instant) -> Result<String, String> {
        if let Some(league) = &self.league {
            return Ok(league.clone());
        }
        if let Some((at, league)) = &self.default_league
            && now.duration_since(*at) < MAX_AGE
        {
            return Ok(league.clone());
        }
        #[derive(Deserialize)]
        struct League {
            id: String,
        }
        let body = self.get(&format!("{NINJA}/leagues"))?;
        let leagues: Vec<League> = serde_json::from_str(&body).map_err(|err| unexpected("league list", err))?;
        let first = leagues.into_iter().next().ok_or("poe.ninja lists no leagues")?.id;
        self.default_league = Some((now, first.clone()));
        Ok(first)
    }

    fn get(&self, url: &str) -> Result<String, String> {
        self.fetch.get(url).map_err(|err| format!("Couldn't reach poe.ninja: {err}"))
    }
}

fn unexpected(what: &str, err: impl std::fmt::Display) -> String {
    format!("poe.ninja sent an unexpected {what} ({err})")
}

fn parse_currency(body: &str, league: &str) -> Result<CurrencyRates, String> {
    #[derive(Deserialize)]
    struct Overview {
        core: Core,
        lines: Vec<Line>,
        items: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Core {
        /// Units of each secondary currency one primary (chaos) buys.
        rates: HashMap<String, f64>,
        primary: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Line {
        id: String,
        /// Price in the primary currency.
        primary_value: f64,
        volume_primary_value: f64,
    }
    #[derive(Deserialize)]
    struct Item {
        id: String,
        name: String,
    }
    let overview: Overview = serde_json::from_str(body).map_err(|err| unexpected("currency overview", err))?;
    if overview.lines.is_empty() {
        return Err(format!(
            "poe.ninja has no prices for the league \"{league}\"; check [prices] league in config.toml"
        ));
    }
    if overview.core.primary != "chaos" {
        return Err(unexpected("currency overview", format!("prices in {}", overview.core.primary)));
    }
    let divine_per_chaos = *overview.core.rates.get("divine").ok_or_else(|| unexpected("currency overview", "no divine rate"))?;
    let names: HashMap<&str, &str> = overview.items.iter().map(|i| (i.id.as_str(), i.name.as_str())).collect();
    let mut lines: Vec<&Line> = overview.lines.iter().filter(|l| l.id != "chaos").collect();
    lines.sort_by(|a, b| b.volume_primary_value.total_cmp(&a.volume_primary_value));
    let rates = lines
        .into_iter()
        .filter_map(|line| {
            Some(CurrencyRate {
                name: (*names.get(line.id.as_str())?).to_owned(),
                chaos: line.primary_value,
                divine: line.primary_value * divine_per_chaos,
            })
        })
        .take(LISTED)
        .collect();
    Ok(CurrencyRates { league: league.to_owned(), rates })
}

/// Percent-encodes a league name for a query string.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
