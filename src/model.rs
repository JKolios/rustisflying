//! Data models: raw aircraft records from the position feed and the
//! enriched display model produced after route/airline lookup.

use serde::Deserialize;

/// Barometric altitude as reported by the feed: either a value in feet or
/// the string `"ground"` for aircraft on the surface.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Altitude {
    Feet(f64),
    /// The payload records the literal string reported by the feed (always
    /// "ground"); only the variant's presence is used.
    Ground(#[allow(dead_code)] String),
}

impl Altitude {
    pub fn feet(&self) -> Option<f64> {
        match self {
            Altitude::Feet(f) => Some(*f),
            Altitude::Ground(_) => None,
        }
    }

    pub fn is_ground(&self) -> bool {
        matches!(self, Altitude::Ground(_))
    }
}

/// A single aircraft as returned by the adsb.lol v2 `/point` API.
/// Only the fields we use are modeled; the rest are ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct Aircraft {
    /// ICAO 24-bit address, hex string.
    pub hex: String,
    /// Callsign as filed, padded with trailing spaces by the feed.
    #[serde(default)]
    pub flight: Option<String>,
    pub lat: f64,
    pub lon: f64,
    #[serde(default)]
    pub alt_baro: Option<Altitude>,
    /// Ground speed in knots.
    #[serde(default)]
    pub gs: Option<f64>,
    /// Track over ground in degrees. Reserved for future renderers
    /// (e-ink compass arrow, web UI).
    #[allow(dead_code)]
    #[serde(default)]
    pub track: Option<f64>,
    /// Registration (tail number).
    #[serde(default)]
    pub r: Option<String>,
    /// Aircraft type code, e.g. "A320".
    #[serde(default)]
    pub t: Option<String>,
    /// Seconds since the last position update.
    #[serde(default)]
    pub seen_pos: Option<f64>,
}

/// Top-level shape of the v2 API response.
#[derive(Debug, Deserialize)]
pub struct PointResponse {
    pub ac: Vec<Aircraft>,
}

/// A resolved airport on either end of a route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AirportRef {
    pub icao: String,
    pub iata: Option<String>,
    pub name: Option<String>,
}

/// The enriched, display-ready description of one aircraft.
#[derive(Debug, Clone)]
pub struct FlightInfo {
    pub callsign: String,
    pub airline: Option<String>,
    pub origin: Option<AirportRef>,
    pub destination: Option<AirportRef>,
    pub registration: Option<String>,
    pub aircraft_type: Option<String>,
    pub altitude_ft: Option<f64>,
    /// Ground speed in km/h (converted from the feed's knots).
    pub ground_speed_kmh: Option<f64>,
    /// Distance from the geofence center in km.
    pub distance_km: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn altitude_parses_feet_and_ground() {
        let feet: Altitude = serde_json::from_str("12450").unwrap();
        assert_eq!(feet.feet(), Some(12450.0));
        assert!(!feet.is_ground());

        let ground: Altitude = serde_json::from_str("\"ground\"").unwrap();
        assert_eq!(ground.feet(), None);
        assert!(ground.is_ground());
    }

    #[test]
    fn aircraft_deserializes_from_feed_shape() {
        let json = r#"{
            "hex": "4691c7",
            "flight": "AEE251  ",
            "lat": 38.0412,
            "lon": 24.0198,
            "alt_baro": 36000,
            "gs": 402.5,
            "track": 182.3,
            "r": "SX-DNB",
            "t": "A320",
            "seen_pos": 0.4
        }"#;
        let ac: Aircraft = serde_json::from_str(json).unwrap();
        assert_eq!(ac.flight.as_deref(), Some("AEE251  "));
        assert_eq!(ac.alt_baro.as_ref().unwrap().feet(), Some(36000.0));
        assert_eq!(ac.t.as_deref(), Some("A320"));
    }
}
