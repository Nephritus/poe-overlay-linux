//! HTTP adapter for the price source: plain GETs with the Overlay's
//! User-Agent, so poe.ninja can tell who is calling and reach us.

use std::time::Duration;

use crate::poe1::prices::Fetch;

const USER_AGENT: &str = concat!("poe-overlay-linux/", env!("CARGO_PKG_VERSION"), " (contact: https://github.com/Nephritus)");

pub struct Http(ureq::Agent);

impl Http {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .user_agent(USER_AGENT)
            .timeout_global(Some(Duration::from_secs(10)))
            .build();
        Self(config.into())
    }
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetch for Http {
    fn get(&self, url: &str) -> Result<String, String> {
        let mut response = self.0.get(url).call().map_err(|err| err.to_string())?;
        response.body_mut().read_to_string().map_err(|err| err.to_string())
    }
}
