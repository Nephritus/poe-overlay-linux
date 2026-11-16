//! HTTP adapter for trade site searches. Cloudflare answers clients that
//! name themselves with 403, so this one sends a browser's User-Agent, as
//! Awakened PoE Trade and Sidekick do.

use std::time::Duration;

use crate::poe1::trade_search::{TradeHttp, TradeResponse};

const BROWSER: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

pub struct TradeSite(ureq::Agent);

impl TradeSite {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .user_agent(BROWSER)
            .timeout_global(Some(Duration::from_secs(10)))
            // A 403 or 429 is an answer with headers to read, not a failure.
            .http_status_as_error(false)
            .build();
        Self(config.into())
    }
}

impl TradeHttp for TradeSite {
    fn send(&self, url: &str, body: Option<&str>) -> Result<TradeResponse, String> {
        let sent = match body {
            Some(body) => self.0.post(url).header("Content-Type", "application/json").send(body),
            None => self.0.get(url).call(),
        };
        let mut response = sent.map_err(|err| err.to_string())?;
        let headers = response
            .headers()
            .iter()
            .filter(|(name, _)| name.as_str().starts_with("x-rate-limit") || *name == "retry-after")
            .filter_map(|(name, value)| Some((name.as_str().to_owned(), value.to_str().ok()?.to_owned())))
            .collect();
        let body = response.body_mut().read_to_string().map_err(|err| err.to_string())?;
        Ok(TradeResponse { status: response.status().as_u16(), headers, body })
    }
}
