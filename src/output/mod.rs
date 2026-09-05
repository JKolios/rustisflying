//! Output sinks.
//!
//! [`FlightOutput`] is the seam for every renderer: the terminal, the web
//! UI's shared state, and the e-paper panel all consume the same
//! [`TickResult`] snapshot.

use crate::model::TickResult;

// Layout + rendering are always compiled (and unit-tested) off the Pi; only
// the hardware worker is feature-gated. In builds that cannot wire the panel
// up, the pipeline is otherwise unused — that's expected, not dead code.
#[cfg_attr(
    not(all(feature = "epaper", target_os = "linux")),
    allow(dead_code)
)]
pub mod epaper;
pub mod terminal;
pub mod web_state;

#[cfg(all(feature = "epaper", target_os = "linux"))]
pub use epaper::hw::EpaperOutput;
pub use terminal::Terminal;
pub use web_state::WebState;

/// A sink for tick results.
pub trait FlightOutput {
    fn emit(&self, result: &TickResult);
}
