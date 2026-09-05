//! Enrichment pipeline: turn a raw [`Aircraft`] from the position feed into
//! a display-ready [`FlightInfo`] by resolving the airline from the callsign
//! and the route from hexdb.io.
//!
//! All enrichment is best-effort: anything unknown degrades to `None` and
//! the renderer decides how to present "we don't know". Results are cached
//! per callsign / ICAO code for the lifetime of the process, which both
//! avoids hammering hexdb on every tick and keeps repeated sightings of the
//! same flight cheap.

use crate::model::{Aircraft, AirportRef, FlightInfo, VerticalDirection};
use std::collections::HashMap;

pub mod airlines;
pub mod hexdb;

pub use hexdb::HexdbClient;

/// km/h per knot.
const KMH_PER_KNOT: f64 = 1.852;

pub struct Enricher {
    hexdb: Option<HexdbClient>,
    route_cache: HashMap<String, Option<String>>,
    airport_cache: HashMap<String, Option<AirportRef>>,
}

impl Enricher {
    pub fn new(hexdb: Option<HexdbClient>) -> Self {
        Self {
            hexdb,
            route_cache: HashMap::new(),
            airport_cache: HashMap::new(),
        }
    }

    /// Build the display model for one aircraft.
    pub async fn enrich(&mut self, ac: &Aircraft, distance_km: f64) -> FlightInfo {
        // The feed pads callsigns with spaces; when absent, fall back to hex.
        let callsign = ac
            .flight
            .as_ref()
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| ac.hex.to_uppercase());

        let airline = airlines::airline_name(&callsign).map(str::to_string);

        // Route lookup is only meaningful for real callsigns.
        let route = if ac.flight.is_some() {
            self.route_for(&callsign).await
        } else {
            None
        };
        let (origin, destination) = match route.as_deref() {
            Some(r) => {
                let legs: Vec<&str> = r.split('-').filter(|s| !s.is_empty()).collect();
                match legs.as_slice() {
                    [first, .., last] => {
                        (self.airport_for(first).await, self.airport_for(last).await)
                    }
                    [only] => (self.airport_for(only).await, None),
                    _ => (None, None),
                }
            }
            None => (None, None),
        };

        FlightInfo {
            callsign,
            airline,
            origin,
            destination,
            registration: ac.r.clone(),
            aircraft_type: ac.t.clone(),
            altitude_ft: ac.alt_baro.as_ref().and_then(|a| a.feet()),
            ground_speed_kmh: ac.gs.map(|gs| gs * KMH_PER_KNOT),
            vertical_direction: ac.baro_rate.map(VerticalDirection::from_rate),
            distance_km,
        }
    }

    async fn route_for(&mut self, callsign: &str) -> Option<String> {
        if let Some(cached) = self.route_cache.get(callsign) {
            return cached.clone();
        }
        let route = match &self.hexdb {
            Some(client) => match client.route(callsign).await {
                Ok(route) => route,
                Err(e) => {
                    eprintln!("warning: route lookup for {callsign} failed: {e:#}");
                    None
                }
            },
            None => None,
        };
        self.route_cache.insert(callsign.to_string(), route.clone());
        route
    }

    async fn airport_for(&mut self, icao: &str) -> Option<AirportRef> {
        let key = icao.to_ascii_uppercase();
        if let Some(cached) = self.airport_cache.get(&key) {
            return cached.clone();
        }
        let airport = match &self.hexdb {
            Some(client) => match client.airport(&key).await {
                Ok(info) => info.map(|i| AirportRef {
                    icao: i.icao.to_ascii_uppercase(),
                    iata: i.iata,
                    name: i.airport,
                }),
                Err(e) => {
                    eprintln!("warning: airport lookup for {key} failed: {e:#}");
                    None
                }
            },
            None => None,
        };
        self.airport_cache.insert(key, airport.clone());
        airport
    }
}
