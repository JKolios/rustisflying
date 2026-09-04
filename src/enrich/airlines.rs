//! Airline identification from a callsign alone.
//!
//! Airline callsigns begin with the airline's three-letter ICAO designator
//! (`DAL123` → `DAL` = Delta Air Lines, `AEE251` → `AEE` = Aegean). No free
//! live API resolves designators to names, so we carry a curated embedded
//! table instead. Extend `AIRLINES` as needed; lookups are exact matches on
//! the first three alphabetic characters of a trimmed callsign.

/// (ICAO designator, airline name) pairs for carriers commonly seen from
/// the geofence plus major international airlines.
const AIRLINES: &[(&str, &str)] = &[
    // Greece and the eastern Mediterranean
    ("AEE", "Aegean Airlines"),
    ("OAL", "Olympic Air"),
    ("THY", "Turkish Airlines"),
    ("MSC", "Air Cairo"),
    ("MSR", "EgyptAir"),
    ("MEA", "Middle East Airlines"),
    ("RJA", "Royal Jordanian"),
    ("AMC", "Air Malta"),
    ("NOS", "Neos"),
    ("SEH", "Sky Express"),
    ("ISR", "Israir"),
    ("AIZ", "Arkia Israeli Airlines"),
    // Europe
    ("DLH", "Lufthansa"),
    ("BAW", "British Airways"),
    ("KLM", "KLM Royal Dutch Airlines"),
    ("AFR", "Air France"),
    ("RYR", "Ryanair"),
    ("EWG", "Eurowings"),
    ("WZZ", "Wizz Air"),
    ("EZY", "easyJet"),
    ("VIR", "Virgin Atlantic"),
    ("SAS", "Scandinavian Airlines"),
    ("AUA", "Austrian Airlines"),
    ("SWR", "Swiss International Air Lines"),
    ("IBE", "Iberia"),
    ("TAP", "TAP Air Portugal"),
    ("ITY", "ITA Airways"),
    ("EIN", "Aer Lingus"),
    ("CFG", "Condor"),
    ("TUI", "TUI Airways"),
    // Middle East / Asia / Oceania
    ("UAE", "Emirates"),
    ("QTR", "Qatar Airways"),
    ("ETD", "Etihad Airways"),
    ("KAC", "Kuwait Airways"),
    ("GFA", "Gulf Air"),
    ("FDB", "flydubai"),
    ("ETH", "Ethiopian Airlines"),
    ("SIA", "Singapore Airlines"),
    ("CPA", "Cathay Pacific"),
    ("ANA", "All Nippon Airways"),
    ("JAL", "Japan Airlines"),
    ("QFA", "Qantas"),
    ("ANZ", "Air New Zealand"),
    // North America
    ("DAL", "Delta Air Lines"),
    ("UAL", "United Airlines"),
    ("AAL", "American Airlines"),
    ("ACA", "Air Canada"),
    ("JBU", "JetBlue Airways"),
];

/// Extract the ICAO airline designator from a callsign: the first three
/// characters, which must all be letters (so tail numbers like `N12345`
/// and other non-airline callsigns correctly yield `None`).
pub fn designator(callsign: &str) -> Option<String> {
    let prefix: String = callsign
        .trim()
        .chars()
        .take(3)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if prefix.len() == 3 && prefix.chars().all(|c| c.is_ascii_alphabetic()) {
        Some(prefix)
    } else {
        None
    }
}

/// Resolve an airline name from a callsign, or `None` if the designator
/// is not in the table (or the callsign has no airline designator).
pub fn airline_name(callsign: &str) -> Option<&'static str> {
    let designator = designator(callsign)?;
    AIRLINES
        .iter()
        .find(|(code, _)| *code == designator)
        .map(|(_, name)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_designator_from_airline_callsigns() {
        assert_eq!(designator("DAL123").as_deref(), Some("DAL"));
        assert_eq!(designator("AEE251").as_deref(), Some("AEE"));
        assert_eq!(designator("  baw16f ").as_deref(), Some("BAW")); // trimmed + uppercased
    }

    #[test]
    fn rejects_non_airline_callsigns() {
        assert_eq!(designator("N12345"), None); // US tail number
        assert_eq!(designator("AB"), None); // too short
        assert_eq!(designator(""), None);
    }

    #[test]
    fn looks_up_airline_names() {
        assert_eq!(airline_name("DAL123"), Some("Delta Air Lines"));
        assert_eq!(airline_name("AEE251"), Some("Aegean Airlines"));
        assert_eq!(airline_name("BAW16F"), Some("British Airways"));
        assert_eq!(airline_name("SEH8HR"), Some("Sky Express"));
        assert_eq!(airline_name("N12345"), None);
        assert_eq!(airline_name("ZZZ999"), None); // not in table
    }
}
