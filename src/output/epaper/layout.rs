//! Wording for one e-paper frame, derived from a [`TickResult`].
//!
//! Deliberately free of any display types: the small-screen phrasing is the
//! part worth unit-testing off the Pi, while [`super::render`] only positions
//! these strings. Compared to the terminal renderer, the registration is
//! dropped (no room) and airport names are trimmed to fit the 264 px panel.

use crate::model::{AirportRef, CompassPoint, TickResult, VerticalDirection};
use chrono::Local;

/// Airport names are capped for the panel: at most this many words...
const AIRPORT_MAX_WORDS: usize = 3;
/// ...and this many characters, so two labels plus the route arrow fit the
/// route line; a cut name is marked with an ellipsis.
const AIRPORT_MAX_CHARS: usize = 14;
/// Airline names are capped to keep the 14 pt line inside the panel.
const AIRLINE_MAX_CHARS: usize = 24;

/// The complete content of one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    /// "HH:MM" local time of the tick, always shown so a stale panel is
    /// obvious at a glance.
    pub stamp: String,
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    Closest {
        callsign: String,
        airline: Option<String>,
        route: Option<Route>,
        /// e.g. "34,000 ft"; `trend` renders as an arrow next to it.
        altitude: Option<String>,
        trend: Option<VerticalDirection>,
        /// e.g. "742 km/h".
        speed: Option<String>,
        heading: Option<CompassPoint>,
        /// e.g. "5.6 km away"; always present.
        distance: String,
        aircraft_type: Option<String>,
    },
    Empty {
        radius_km: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    Between {
        origin: AirportLabel,
        destination: AirportLabel,
    },
    /// Only one end of the route is known (mirrors the terminal's "near X").
    Near(AirportLabel),
}

/// How one airport is shown. The trimmed name is preferred; the compact code
/// (IATA, else ICAO) is the fallback for the narrow route line.
#[derive(Debug, Clone, PartialEq)]
pub struct AirportLabel {
    pub name: Option<String>,
    pub code: String,
}

/// Build the screen content for one tick.
pub fn layout(result: &TickResult) -> Screen {
    let stamp = Local::now().format("%H:%M").to_string();
    let body = match result {
        TickResult::Closest { flight } => Body::Closest {
            callsign: flight.callsign.clone(),
            airline: flight.airline.as_deref().map(trim_airline),
            route: route_line(&flight.origin, &flight.destination),
            altitude: flight
                .altitude_ft
                .map(|a| format!("{} ft", group_thousands(a.round() as i64))),
            trend: flight.vertical_direction,
            speed: flight
                .ground_speed_kmh
                .map(|s| format!("{} km/h", group_thousands(s.round() as i64))),
            heading: flight.heading,
            distance: format!("{:.1} km away", flight.distance_km),
            aircraft_type: flight.aircraft_type.clone(),
        },
        TickResult::Empty { radius_km } => Body::Empty {
            radius_km: *radius_km,
        },
    };
    Screen { stamp, body }
}

fn route_line(origin: &Option<AirportRef>, destination: &Option<AirportRef>) -> Option<Route> {
    match (origin, destination) {
        (Some(o), Some(d)) => Some(Route::Between {
            origin: airport_label(o),
            destination: airport_label(d),
        }),
        (Some(a), None) | (None, Some(a)) => Some(Route::Near(airport_label(a))),
        (None, None) => None,
    }
}

fn airport_label(airport: &AirportRef) -> AirportLabel {
    AirportLabel {
        name: airport.name.as_deref().map(trim_airport),
        code: airport
            .iata
            .clone()
            .unwrap_or_else(|| airport.icao.clone()),
    }
}

/// Format an integer with thousands separators ("34000" -> "34,000"); big
/// altitudes are much easier to read on the panel with them.
fn group_thousands(n: i64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Trim an airline name to the panel width, marking a cut with an ellipsis.
fn trim_airline(name: &str) -> String {
    let collapsed: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= AIRLINE_MAX_CHARS {
        return collapsed;
    }
    let stem: String = collapsed.chars().take(AIRLINE_MAX_CHARS - 1).collect();
    format!("{}…", stem.trim_end())
}

/// Trim an airport name for the panel: the first [`AIRPORT_MAX_WORDS`] words
/// that fit within [`AIRPORT_MAX_CHARS`] characters. Words are kept whole so
/// the city name — the informative part — survives; a cut is marked with an
/// ellipsis.
pub fn trim_airport(name: &str) -> String {
    let mut out = String::new();
    for word in name.split_whitespace().take(AIRPORT_MAX_WORDS) {
        let candidate = if out.is_empty() {
            word.to_string()
        } else {
            format!("{out} {word}")
        };
        if candidate.chars().count() > AIRPORT_MAX_CHARS {
            break;
        }
        out = candidate;
    }
    let full: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if full != out {
        // The cap dropped or cut a word: mark it.
        return format!("{out}…");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AirportRef, FlightInfo};

    fn airport(name: Option<&str>, iata: Option<&str>) -> AirportRef {
        AirportRef {
            icao: "KBOS".into(),
            iata: iata.map(str::to_string),
            name: name.map(str::to_string),
        }
    }

    #[test]
    fn trim_airport_caps_words_and_chars() {
        assert_eq!(trim_airport("Athens"), "Athens");
        // Within the cap: untouched, even at three words.
        assert_eq!(trim_airport("New York John"), "New York John");
        // Character cap drops whole words; the cut is marked.
        assert_eq!(trim_airport("Athens International Airport"), "Athens…");
        assert_eq!(trim_airport("San Francisco International"), "San Francisco…");
        assert_eq!(trim_airport("Los Angeles International"), "Los Angeles…");
        // Three-word limit beyond the character cap.
        assert_eq!(
            trim_airport("New York John F Kennedy International"),
            "New York John…"
        );
        // Whitespace runs collapse; the character cap still applies.
        assert_eq!(trim_airport("Los   Angeles  Intl"), "Los Angeles…");
    }

    #[test]
    fn thousands_separators_group_from_the_right() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(6525), "6,525");
        assert_eq!(group_thousands(34000), "34,000");
        assert_eq!(group_thousands(1234567), "1,234,567");
    }

    #[test]
    fn trim_airline_caps_with_ellipsis() {
        assert_eq!(trim_airline("Aegean Airlines"), "Aegean Airlines");
        // 23 chars fits the 24-char cap; 26 does not.
        assert_eq!(trim_airline("China Southern Airlines"), "China Southern Airlines");
        assert_eq!(
            trim_airline("Aeroflot Russian Airlines"),
            "Aeroflot Russian Airlin…"
        );
        // Whitespace runs collapse before measuring.
        assert_eq!(trim_airline("Air  France"), "Air France");
    }

    #[test]
    fn layout_closest_carries_all_fields() {
        let flight = FlightInfo {
            callsign: "AEE166".into(),
            airline: Some("Aegean Airlines".into()),
            origin: Some(airport(Some("Athens International Airport"), Some("ATH"))),
            destination: Some(airport(Some("London Heathrow"), Some("LHR"))),
            registration: Some("SX-OBN".into()),
            aircraft_type: Some("AT76".into()),
            altitude_ft: Some(6525.0),
            ground_speed_kmh: Some(296.0),
            vertical_direction: Some(VerticalDirection::Descending),
            heading: Some(CompassPoint::Northeast),
            distance_km: 5.6,
        };
        let screen = layout(&TickResult::Closest {
            flight: Box::new(flight),
        });
        let Body::Closest {
            callsign,
            airline,
            route,
            altitude,
            trend,
            speed,
            heading,
            distance,
            aircraft_type,
        } = screen.body
        else {
            panic!("expected Closest");
        };
        assert_eq!(callsign, "AEE166");
        assert_eq!(airline.as_deref(), Some("Aegean Airlines"));
        assert!(matches!(route, Some(Route::Between { .. })));
        assert_eq!(altitude.as_deref(), Some("6,525 ft"));
        assert_eq!(trend, Some(VerticalDirection::Descending));
        assert_eq!(speed.as_deref(), Some("296 km/h"));
        assert_eq!(heading, Some(CompassPoint::Northeast));
        assert_eq!(distance, "5.6 km away");
        assert_eq!(aircraft_type.as_deref(), Some("AT76"));
        assert_eq!(screen.stamp.chars().count(), 5);
    }

    #[test]
    fn layout_closest_with_unknowns_omits_route() {
        let flight = FlightInfo {
            callsign: "TEST1".into(),
            airline: None,
            origin: None,
            destination: None,
            registration: None,
            aircraft_type: None,
            altitude_ft: None,
            ground_speed_kmh: None,
            vertical_direction: None,
            heading: None,
            distance_km: 1.0,
        };
        let screen = layout(&TickResult::Closest {
            flight: Box::new(flight),
        });
        let Body::Closest { route, distance, .. } = screen.body else {
            panic!("expected Closest");
        };
        assert_eq!(route, None);
        assert_eq!(distance, "1.0 km away");
    }

    #[test]
    fn layout_single_known_airport_is_near() {
        let flight = FlightInfo {
            callsign: "TEST1".into(),
            airline: None,
            origin: Some(airport(None, Some("BOS"))),
            destination: None,
            registration: None,
            aircraft_type: None,
            altitude_ft: None,
            ground_speed_kmh: None,
            vertical_direction: None,
            heading: None,
            distance_km: 1.0,
        };
        let screen = layout(&TickResult::Closest {
            flight: Box::new(flight),
        });
        let Body::Closest { route, .. } = screen.body else {
            panic!("expected Closest");
        };
        // No name on the airport: the IATA code is the label.
        let expected = Route::Near(AirportLabel {
            name: None,
            code: "BOS".into(),
        });
        assert_eq!(route, Some(expected));
    }

    #[test]
    fn layout_empty_keeps_radius() {
        let screen = layout(&TickResult::Empty { radius_km: 30.0 });
        assert_eq!(screen.body, Body::Empty { radius_km: 30.0 });
    }
}
