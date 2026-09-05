//! E-paper output for the Waveshare 2.7inch e-Paper HAT (B) tri-color panel.
//!
//! [`layout`] turns a tick into plain-text screen content, [`render`] draws
//! it into the two pixel planes, and [`hw`] (behind `--features epaper`,
//! Linux only) ships frames to the panel from a dedicated worker thread.

pub mod layout;
pub mod render;

#[cfg(all(feature = "epaper", target_os = "linux"))]
pub mod hw;
