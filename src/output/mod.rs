//! Output sinks.
//!
//! [`FlightOutput`] is the seam for every renderer: the terminal, the web
//! UI's shared state, and the planned E-ink image generator all consume the
//! same [`TickResult`] snapshot.

use crate::model::TickResult;

pub mod terminal;
pub mod web_state;

pub use terminal::Terminal;
pub use web_state::WebState;

/// A sink for tick results.
pub trait FlightOutput {
    fn emit(&self, result: &TickResult);
}
