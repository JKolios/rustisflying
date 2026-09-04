//! Output sinks.
//!
//! [`FlightOutput`] is the seam for the planned renderers: a text-based web
//! UI and an E-ink image generator will be additional implementations.

use crate::model::FlightInfo;

pub mod terminal;

pub use terminal::Terminal;

/// Where a tick's result is rendered.
pub trait FlightOutput {
    fn render_closest(&self, info: &FlightInfo);
    fn render_empty(&self, radius_km: f64);
}
