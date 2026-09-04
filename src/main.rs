//! rustisflying: watch the sky above home.
//!
//! On a timer, queries a live position feed for aircraft inside a circular
//! geofence around the configured home coordinates, enriches the closest one
//! (airline from its callsign, route from hexdb.io), and prints what it
//! found. Exit with Ctrl+C.

mod config;
mod enrich;
mod geo;
mod model;
mod output;
mod provider;

use config::Config;
use enrich::{Enricher, HexdbClient};
use geo::{Geofence, closest};
use model::Aircraft;
use output::{FlightOutput, Terminal};
use provider::{AdsbLolClient, FlightProvider};
use std::time::Duration;
use tokio::time::{MissedTickBehavior, interval};

/// Minimum polling interval, guarding the free feed's ~1 request/second
/// courtesy limit even if the config asks for something faster.
const MIN_INTERVAL_SECONDS: u64 = 5;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    if config.home.latitude == 0.0 && config.home.longitude == 0.0 {
        eprintln!("warning: home coordinates are 0,0 — edit config.toml to set your location");
    }

    let fence = Geofence::new(
        config.home.latitude,
        config.home.longitude,
        config.home.radius_km,
    );
    let adsb = AdsbLolClient::new(&config.api.adsb_base_url)?;
    let hexdb = HexdbClient::new(&config.api.hexdb_base_url)?;
    let mut enricher = Enricher::new(Some(hexdb));
    let output = Terminal;

    let interval_seconds = config.polling.interval_seconds.max(MIN_INTERVAL_SECONDS);
    let mut tick = interval(Duration::from_secs(interval_seconds));
    // If a tick overruns (slow network), wait for the next slot rather than
    // bursting to catch up.
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    println!(
        "rustisflying: watching a {:.0} km circle around {:.5}, {:.5}, every {} s",
        fence.radius_km, fence.latitude, fence.longitude, interval_seconds
    );

    loop {
        tick.tick().await; // first tick completes immediately
        run_tick(&adsb, &mut enricher, &output, &fence, &config).await;
    }
}

async fn run_tick(
    provider: &impl FlightProvider,
    enricher: &mut Enricher,
    output: &impl FlightOutput,
    fence: &Geofence,
    config: &Config,
) {
    let aircraft = match provider.aircraft_near(fence).await {
        Ok(aircraft) => aircraft,
        Err(e) => {
            eprintln!("error fetching aircraft positions: {e:#}");
            return;
        }
    };

    let candidates: Vec<Aircraft> = aircraft
        .into_iter()
        .filter(|ac| fence.contains(ac.lat, ac.lon))
        .filter(|ac| {
            config.filter.include_ground
                || !ac.alt_baro.as_ref().is_some_and(|alt| alt.is_ground())
        })
        .filter(|ac| {
            ac.seen_pos
                .is_none_or(|seen| seen <= config.filter.max_seen_pos_seconds)
        })
        .collect();

    match closest(fence, &candidates) {
        Some(ac) => {
            let distance = fence.distance_km(ac.lat, ac.lon);
            let info = enricher.enrich(ac, distance).await;
            output.render_closest(&info);
        }
        None => output.render_empty(fence.radius_km),
    }
}
