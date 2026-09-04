//! Client for hexdb.io, a free community metadata API used to enrich a
//! callsign with its route and to resolve airport ICAO codes to names.
//!
//! Both lookups are best-effort: coverage is crowdsourced, so a 404 or an
//! empty result simply means "unknown" and must not fail the tick.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub struct HexdbClient {
    http: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
pub struct RouteResponse {
    /// Route as `"EGPH-KBOS"` (some entries have more legs, e.g. `"A-B-C"`).
    #[serde(default)]
    pub route: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AirportInfo {
    #[serde(default)]
    pub icao: String,
    #[serde(default)]
    pub iata: Option<String>,
    #[serde(default)]
    pub airport: Option<String>,
}

impl HexdbClient {
    pub fn new(base_url: &str) -> Result<Self> {
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .context("building HTTP client")?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// The filed route for a callsign as `"AAAA-BBBB"`, or `None` if hexdb
    /// doesn't know it (404) or the response has no route field.
    pub async fn route(&self, callsign: &str) -> Result<Option<String>> {
        let url = format!("{}/api/v1/route/icao/{callsign}", self.base_url);
        let response = self.http.get(&url).send().await?;
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        if !response.status().is_success() {
            anyhow::bail!("hexdb returned {} for route {callsign}", response.status());
        }
        let route: RouteResponse = response.json().await?;
        Ok(route.route)
    }

    /// Airport metadata for an ICAO code, or `None` if unknown.
    pub async fn airport(&self, icao: &str) -> Result<Option<AirportInfo>> {
        let url = format!("{}/api/v1/airport/icao/{icao}", self.base_url);
        let response = self.http.get(&url).send().await?;
        if response.status().as_u16() == 404 {
            return Ok(None);
        }
        if !response.status().is_success() {
            anyhow::bail!("hexdb returned {} for airport {icao}", response.status());
        }
        let info: AirportInfo = response.json().await?;
        Ok(Some(info))
    }
}
