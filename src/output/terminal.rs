//! Plain-text terminal rendering, one block per tick.

use super::FlightOutput;
use crate::model::{AirportRef, FlightInfo, TickResult};
use chrono::Local;

pub struct Terminal;

impl FlightOutput for Terminal {
    fn emit(&self, result: &TickResult) {
        match result {
            TickResult::Closest { flight } => render_closest(flight),
            TickResult::Empty { radius_km } => render_empty(*radius_km),
        }
    }
}

fn render_closest(info: &FlightInfo) {
    let stamp = Local::now().format("%H:%M:%S");
    let mut line = format!("[{stamp}] {}", info.callsign);
    if let Some(airline) = &info.airline {
        line.push_str(&format!(" · {airline}"));
    }
    println!("{line}");

    match (&info.origin, &info.destination) {
        (Some(origin), Some(destination)) => println!(
            "    {} → {}",
            format_airport(origin),
            format_airport(destination)
        ),
        (Some(airport), None) | (None, Some(airport)) => {
            println!("    near {}", format_airport(airport))
        }
        (None, None) => println!("    route unknown"),
    }

    let mut details = Vec::new();
    details.push(format!("{:.1} km away", info.distance_km));
    if let Some(alt) = info.altitude_ft {
        details.push(format!("{} ft", alt.round() as i64));
    }
    if let Some(speed) = info.ground_speed_kmh {
        details.push(format!("{} km/h", speed.round() as i64));
    }
    if let Some(kind) = &info.aircraft_type {
        details.push(kind.clone());
    }
    if let Some(reg) = &info.registration {
        details.push(reg.clone());
    }
    println!("    {}", details.join(" · "));
}

fn render_empty(radius_km: f64) {
    let stamp = Local::now().format("%H:%M:%S");
    println!("[{stamp}] No aircraft within {radius_km:.0} km of home.");
}

fn format_airport(airport: &AirportRef) -> String {
    match (&airport.name, &airport.iata) {
        (Some(name), Some(iata)) => format!("{name} ({iata})"),
        (Some(name), None) => name.clone(),
        (None, Some(iata)) => format!("{iata} ({icao})", icao = airport.icao),
        (None, None) => airport.icao.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn airport(name: Option<&str>, iata: Option<&str>) -> AirportRef {
        AirportRef {
            icao: "KBOS".into(),
            iata: iata.map(str::to_string),
            name: name.map(str::to_string),
        }
    }

    #[test]
    fn formats_airport_variants() {
        assert_eq!(format_airport(&airport(Some("Logan International Airport"), Some("BOS"))),
            "Logan International Airport (BOS)");
        assert_eq!(format_airport(&airport(Some("Logan International Airport"), None)),
            "Logan International Airport");
        assert_eq!(format_airport(&airport(None, Some("BOS"))), "BOS (KBOS)");
        assert_eq!(format_airport(&airport(None, None)), "KBOS");
    }

    #[test]
    fn terminal_emits_both_variants_without_panicking() {
        let terminal = Terminal;
        terminal.emit(&TickResult::Empty { radius_km: 30.0 });
        terminal.emit(&TickResult::Closest {
            flight: Box::new(FlightInfo {
                callsign: "TEST1".into(),
                airline: None,
                origin: None,
                destination: None,
                registration: None,
                aircraft_type: None,
                altitude_ft: None,
                ground_speed_kmh: None,
                distance_km: 1.0,
            }),
        });
    }
}
