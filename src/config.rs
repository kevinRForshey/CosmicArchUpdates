//! Persisted applet settings, stored via `cosmic-config`.

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};

use crate::updates::Source;

/// Selectable check intervals, in minutes.
pub const INTERVALS_MINUTES: [u32; 5] = [5, 15, 30, 60, 120];

#[derive(Debug, Clone, PartialEq, CosmicConfigEntry, serde::Deserialize, serde::Serialize)]
#[version = 1]
pub struct AppletConfig {
    /// Which update sources are counted and applied.
    pub source: Source,
    /// How often to poll for updates.
    pub interval_minutes: u32,
}

impl Default for AppletConfig {
    fn default() -> Self {
        Self {
            source: Source::Both,
            interval_minutes: 30,
        }
    }
}
