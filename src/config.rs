//! Application configuration, loaded from `config.toml` next to where the
//! program is run (i.e. the project root under `cargo run`).
//!
//! Every section has defaults matching the shipped `config.toml`, so a
//! missing or partial file still yields a working program.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

const DEFAULT_CONFIG: &str = "config.toml";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub home: Home,
    pub polling: Polling,
    pub api: Api,
    pub filter: Filter,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Home {
    pub latitude: f64,
    pub longitude: f64,
    pub radius_km: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Polling {
    pub interval_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Api {
    pub adsb_base_url: String,
    pub hexdb_base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Filter {
    pub include_ground: bool,
    pub max_seen_pos_seconds: f64,
}

impl Default for Home {
    fn default() -> Self {
        Self {
            latitude: 0.0,
            longitude: 0.0,
            radius_km: 30.0,
        }
    }
}

impl Default for Polling {
    fn default() -> Self {
        Self {
            interval_seconds: 15,
        }
    }
}

impl Default for Api {
    fn default() -> Self {
        Self {
            adsb_base_url: "https://api.adsb.lol".into(),
            hexdb_base_url: "https://hexdb.io".into(),
        }
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            include_ground: false,
            max_seen_pos_seconds: 90.0,
        }
    }
}

impl Config {
    /// Load `config.toml` from the current directory, falling back to
    /// defaults for the whole file or any missing field within it.
    pub fn load() -> Result<Self> {
        let path = Path::new(DEFAULT_CONFIG);
        if !path.exists() {
            eprintln!(
                "warning: {DEFAULT_CONFIG} not found, using built-in defaults \
                 (edit config.toml to set your home coordinates)"
            );
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let config: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shipped_config() {
        let text = std::fs::read_to_string(DEFAULT_CONFIG).unwrap();
        let config: Config = toml::from_str(&text).unwrap();
        assert!((config.home.latitude - 38.0226).abs() < 1e-3);
        assert!((config.home.longitude - 24.0059).abs() < 1e-3);
        assert_eq!(config.polling.interval_seconds, 15);
        assert!(!config.filter.include_ground);
    }

    #[test]
    fn empty_config_gives_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.home.latitude, 0.0);
        assert_eq!(config.home.radius_km, 30.0);
        assert_eq!(config.polling.interval_seconds, 15);
        assert_eq!(config.api.adsb_base_url, "https://api.adsb.lol");
        assert_eq!(config.filter.max_seen_pos_seconds, 90.0);
    }

    #[test]
    fn partial_config_keeps_defaults_elsewhere() {
        let config: Config = toml::from_str("[home]\nradius_km = 50").unwrap();
        assert_eq!(config.home.radius_km, 50.0);
        assert_eq!(config.home.latitude, 0.0); // default
        assert_eq!(config.polling.interval_seconds, 15); // default
    }
}
