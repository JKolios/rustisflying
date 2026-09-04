//! Client for the free, keyless adsb.lol v2 API.
//!
//! Endpoint used: `GET {base}/v2/point/{lat}/{lon}/{radius_nm}` — the server
//! filters aircraft to a circle around the given point, which maps directly
//! onto our geofence. Be polite: the feed asks for at most ~1 request/second,
//! which the default 15 s polling interval respects easily.

use super::FlightProvider;
use crate::geo::Geofence;
use crate::model::{Aircraft, PointResponse};
use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub struct AdsbLolClient {
    http: Client,
    base_url: String,
}

impl AdsbLolClient {
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
}

impl FlightProvider for AdsbLolClient {
    async fn aircraft_near(&self, fence: &Geofence) -> Result<Vec<Aircraft>> {
        let url = format!(
            "{}/v2/point/{:.5}/{:.5}/{}",
            self.base_url,
            fence.latitude,
            fence.longitude,
            fence.radius_nm()
        );
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .context("requesting aircraft positions")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("adsb.lol returned {status}: {}", body.trim());
        }
        let parsed: PointResponse = response
            .json()
            .await
            .context("decoding aircraft positions")?;
        Ok(parsed.ac)
    }
}
