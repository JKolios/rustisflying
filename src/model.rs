//! Data models: raw aircraft records from the position feed and the
//! enriched display model produced after route/airline lookup.

use serde::{Deserialize, Serialize};

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
    /// Barometric vertical rate in ft/min (negative = descending).
    #[serde(default)]
    pub baro_rate: Option<f64>,
    /// Track over ground in degrees true.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirportRef {
    pub icao: String,
    pub iata: Option<String>,
    pub name: Option<String>,
}

/// Vertical rates within ±this many ft/min count as level flight.
pub const LEVEL_FLIGHT_THRESHOLD_FPM: f64 = 250.0;

/// Whether the aircraft is climbing, descending, or holding altitude,
/// derived from its reported vertical rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalDirection {
    Ascending,
    Descending,
    Level,
}

impl VerticalDirection {
    pub fn from_rate(rate_fpm: f64) -> Self {
        if rate_fpm > LEVEL_FLIGHT_THRESHOLD_FPM {
            VerticalDirection::Ascending
        } else if rate_fpm < -LEVEL_FLIGHT_THRESHOLD_FPM {
            VerticalDirection::Descending
        } else {
            VerticalDirection::Level
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            VerticalDirection::Ascending => "ascending",
            VerticalDirection::Descending => "descending",
            VerticalDirection::Level => "level flight",
        }
    }
}

/// The eight points of the compass, derived from an aircraft's track angle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompassPoint {
    North,
    Northeast,
    East,
    Southeast,
    South,
    Southwest,
    West,
    Northwest,
}

impl CompassPoint {
    /// Quantize a heading in degrees true into the nearest 45° sector.
    pub fn from_degrees(deg: f64) -> Self {
        let sector = ((deg.rem_euclid(360.0) + 22.5) / 45.0) as u8 % 8;
        match sector {
            0 => CompassPoint::North,
            1 => CompassPoint::Northeast,
            2 => CompassPoint::East,
            3 => CompassPoint::Southeast,
            4 => CompassPoint::South,
            5 => CompassPoint::Southwest,
            6 => CompassPoint::West,
            _ => CompassPoint::Northwest,
        }
    }

    /// An arrow pointing in this direction, for heading displays.
    pub fn arrow(&self) -> &'static str {
        match self {
            CompassPoint::North => "↑",
            CompassPoint::Northeast => "↗",
            CompassPoint::East => "→",
            CompassPoint::Southeast => "↘",
            CompassPoint::South => "↓",
            CompassPoint::Southwest => "↙",
            CompassPoint::West => "←",
            CompassPoint::Northwest => "↖",
        }
    }

    /// The usual two-letter abbreviation ("NE", "SW", …).
    pub fn abbrev(&self) -> &'static str {
        match self {
            CompassPoint::North => "N",
            CompassPoint::Northeast => "NE",
            CompassPoint::East => "E",
            CompassPoint::Southeast => "SE",
            CompassPoint::South => "S",
            CompassPoint::Southwest => "SW",
            CompassPoint::West => "W",
            CompassPoint::Northwest => "NW",
        }
    }
}

/// The enriched, display-ready description of one aircraft.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Climb/descent trend, derived from the feed's barometric rate.
    pub vertical_direction: Option<VerticalDirection>,
    /// Heading as the nearest compass point, derived from the track angle.
    pub heading: Option<CompassPoint>,
    /// Distance from the geofence center in km.
    pub distance_km: f64,
}

/// The outcome of one polling tick: what the tracker currently knows.
/// This is the unit every output sink (terminal, web UI, future E-ink
/// renderer) consumes, and the JSON schema of the web API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TickResult {
    /// The aircraft closest to home this tick (boxed to keep the enum small).
    Closest { flight: Box<FlightInfo> },
    /// No aircraft in the geofence this tick.
    Empty { radius_km: f64 },
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
            "baro_rate": -1344,
            "track": 182.3,
            "r": "SX-DNB",
            "t": "A320",
            "seen_pos": 0.4
        }"#;
        let ac: Aircraft = serde_json::from_str(json).unwrap();
        assert_eq!(ac.flight.as_deref(), Some("AEE251  "));
        assert_eq!(ac.alt_baro.as_ref().unwrap().feet(), Some(36000.0));
        assert_eq!(ac.baro_rate, Some(-1344.0));
        assert_eq!(ac.t.as_deref(), Some("A320"));
    }

    #[test]
    fn vertical_direction_from_rate() {
        assert_eq!(VerticalDirection::from_rate(1500.0), VerticalDirection::Ascending);
        assert_eq!(VerticalDirection::from_rate(-1344.0), VerticalDirection::Descending);
        assert_eq!(VerticalDirection::from_rate(0.0), VerticalDirection::Level);
        // The deadband is inclusive: exactly ±threshold still reads as level.
        assert_eq!(VerticalDirection::from_rate(LEVEL_FLIGHT_THRESHOLD_FPM), VerticalDirection::Level);
        assert_eq!(VerticalDirection::from_rate(-LEVEL_FLIGHT_THRESHOLD_FPM), VerticalDirection::Level);
        assert_eq!(
            VerticalDirection::from_rate(LEVEL_FLIGHT_THRESHOLD_FPM + 1.0),
            VerticalDirection::Ascending
        );
    }

    #[test]
    fn compass_point_from_degrees() {
        assert_eq!(CompassPoint::from_degrees(0.0), CompassPoint::North);
        assert_eq!(CompassPoint::from_degrees(90.0), CompassPoint::East);
        assert_eq!(CompassPoint::from_degrees(182.3), CompassPoint::South);
        // Sectors are centered on each point: ±22.5° around it.
        assert_eq!(CompassPoint::from_degrees(22.4), CompassPoint::North);
        assert_eq!(CompassPoint::from_degrees(22.5), CompassPoint::Northeast);
        assert_eq!(CompassPoint::from_degrees(359.9), CompassPoint::North);
        // Out-of-range and negative angles normalize onto the compass rose.
        assert_eq!(CompassPoint::from_degrees(360.0), CompassPoint::North);
        assert_eq!(CompassPoint::from_degrees(-45.0), CompassPoint::Northwest);
    }

    #[test]
    fn tick_result_serializes_for_the_api() {
        let empty = serde_json::to_value(TickResult::Empty { radius_km: 30.0 }).unwrap();
        assert_eq!(
            empty,
            serde_json::json!({"status": "empty", "radius_km": 30.0})
        );

        let flight = FlightInfo {
            callsign: "AEE166".into(),
            airline: Some("Aegean Airlines".into()),
            origin: Some(AirportRef {
                icao: "LGAV".into(),
                iata: Some("ATH".into()),
                name: Some("Athens International Airport".into()),
            }),
            destination: None,
            registration: Some("SX-OBN".into()),
            aircraft_type: Some("AT76".into()),
            altitude_ft: Some(6525.0),
            ground_speed_kmh: Some(296.0),
            vertical_direction: Some(VerticalDirection::Descending),
            heading: Some(CompassPoint::Northeast),
            distance_km: 5.6,
        };
        let closest = serde_json::to_value(TickResult::Closest {
            flight: Box::new(flight),
        })
        .unwrap();
        assert_eq!(closest["status"], "closest");
        assert_eq!(closest["flight"]["callsign"], "AEE166");
        assert_eq!(closest["flight"]["origin"]["iata"], "ATH");
        assert_eq!(closest["flight"]["vertical_direction"], "descending");
        assert_eq!(closest["flight"]["heading"], "northeast");
        assert_eq!(closest["flight"]["destination"], serde_json::Value::Null);
    }
}
