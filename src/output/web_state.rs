//! Shared state between the polling loop and the web server: the latest
//! tick result, readable by HTTP handlers.

use super::FlightOutput;
use crate::model::TickResult;
use std::sync::{Arc, Mutex};

pub struct WebState {
    latest: Mutex<Option<TickResult>>,
}

impl WebState {
    pub fn new() -> Self {
        Self {
            latest: Mutex::new(None),
        }
    }

    /// Snapshot of the latest tick (`None` before the first one completes).
    pub fn latest(&self) -> Option<TickResult> {
        self.latest.lock().unwrap().clone()
    }
}

impl Default for WebState {
    fn default() -> Self {
        Self::new()
    }
}

impl FlightOutput for WebState {
    fn emit(&self, result: &TickResult) {
        *self.latest.lock().unwrap() = Some(result.clone());
    }
}

impl FlightOutput for Arc<WebState> {
    fn emit(&self, result: &TickResult) {
        self.as_ref().emit(result);
    }
}
