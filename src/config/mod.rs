//! Persistent configuration.

use serde::{Deserialize, Serialize};

/// Application configuration persisted to disk.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config;

/// Load configuration from the default path.
pub fn load() -> Config {
    todo!()
}

/// Save configuration to the default path.
pub fn save(_config: &Config) {
    todo!()
}
