//! Live aircraft position providers.
//!
//! [`FlightProvider`] is the seam where alternative feeds (OpenSky,
//! FlightRadar24, aviationstack, a local dump1090 receiver, ...) can be
//! plugged in later without touching the rest of the program.

use crate::geo::Geofence;
use crate::model::Aircraft;
use anyhow::Result;

pub mod adsb_lol;

pub use adsb_lol::AdsbLolClient;

/// A source of live aircraft positions.
pub trait FlightProvider {
    /// All aircraft currently reporting within `fence`'s radius.
    async fn aircraft_near(&self, fence: &Geofence) -> Result<Vec<Aircraft>>;
}
