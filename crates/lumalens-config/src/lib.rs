//! Persistent configuration for LumaLens.
#![warn(missing_docs)]

pub mod preset;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

use lumalens_common::FlipMethod;
use serde::{Deserialize, Serialize};
use tracing::warn;

const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
/// Errors that can occur when loading or saving configuration.
pub enum ConfigError {
    /// Filesystem I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// TOML serialization failure.
    #[error("serialization error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Return the LumaLens configuration directory (`$XDG_CONFIG_HOME/lumalens`).
pub fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("lumalens")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config").join("lumalens")
    } else {
        PathBuf::from(".config").join("lumalens")
    }
}

fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Top-level application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Schema version for forward/backward migration.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Camera selection and resolution.
    #[serde(default)]
    pub camera: CameraSettings,
    /// Image adjustment values.
    #[serde(default)]
    pub image: ImageSettings,
    /// Virtual camera output state.
    #[serde(default)]
    pub virtual_camera: VirtualCameraSettings,
    /// Per-stage effect ordering.
    #[serde(default)]
    pub effects: EffectsSettings,
    /// AI-powered effect settings.
    #[serde(default)]
    pub ai: AiSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            camera: CameraSettings::default(),
            image: ImageSettings::default(),
            virtual_camera: VirtualCameraSettings::default(),
            effects: EffectsSettings::default(),
            ai: AiSettings::default(),
        }
    }
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

/// Persisted camera selection and capture settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraSettings {
    /// Device path of the last-selected camera.
    #[serde(default)]
    pub selected_id: String,
    /// Last-used frame width.
    #[serde(default = "default_resolution_width")]
    pub resolution_width: u32,
    /// Last-used frame height.
    #[serde(default = "default_resolution_height")]
    pub resolution_height: u32,
    /// Last-used frame rate.
    #[serde(default = "default_fps")]
    pub fps: u32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            selected_id: String::new(),
            resolution_width: 1280,
            resolution_height: 720,
            fps: 30,
        }
    }
}

fn default_resolution_width() -> u32 {
    1280
}
fn default_resolution_height() -> u32 {
    720
}
fn default_fps() -> u32 {
    30
}

/// Persisted image adjustment values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSettings {
    /// Brightness (−1.0 to 1.0).
    #[serde(default)]
    pub brightness: f64,
    /// Contrast (0.0 to 2.0).
    #[serde(default = "default_one")]
    pub contrast: f64,
    /// Saturation (0.0 to 2.0).
    #[serde(default = "default_one")]
    pub saturation: f64,
    /// Flip / rotation transform.
    #[serde(default)]
    pub flip: FlipMethod,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            flip: FlipMethod::None,
        }
    }
}

fn default_one() -> f64 {
    1.0
}

/// Persisted virtual camera state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VirtualCameraSettings {
    /// Whether the virtual camera was last enabled.
    #[serde(default)]
    pub enabled: bool,
}

/// Persisted per-stage effect ordering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EffectsSettings {
    /// Maps stage name to ordered list of effect IDs.
    #[serde(default)]
    pub order: BTreeMap<String, Vec<String>>,
}

/// Persisted AI effect settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    /// Whether background blur is enabled.
    #[serde(default = "default_true")]
    pub background_blur: bool,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            background_blur: true,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Load configuration from the default path.
///
/// Returns `Config::default()` if the file is missing or malformed.
/// A malformed file is moved to `config.toml.broken.{timestamp}`.
pub fn load() -> Config {
    let path = config_path();

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Config::default(),
        Err(e) => {
            warn!("failed to read config at {}: {e}", path.display());
            return Config::default();
        }
    };

    let raw: toml::Value = match content.parse() {
        Ok(v) => v,
        Err(e) => {
            warn!("malformed config at {}: {e}", path.display());
            move_broken(&path);
            return Config::default();
        }
    };

    let migrated = migrate(raw);

    match migrated.try_into() {
        Ok(config) => config,
        Err(e) => {
            warn!("failed to deserialize config at {}: {e}", path.display());
            move_broken(&path);
            Config::default()
        }
    }
}

fn move_broken(path: &std::path::Path) {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let broken_name = format!(
        "{}.broken.{timestamp}",
        path.file_name().unwrap_or_default().to_string_lossy()
    );
    let broken_path = path.with_file_name(broken_name);
    if let Err(e) = fs::rename(path, &broken_path) {
        warn!(
            "failed to move broken config to {}: {e}",
            broken_path.display()
        );
    }
}

/// Save configuration atomically (write to temp, fsync, rename).
pub fn save(config: &Config) -> Result<(), ConfigError> {
    let path = config_path();
    let dir = path.parent().expect("config path must have a parent");
    fs::create_dir_all(dir)?;

    let content = toml::to_string_pretty(config)?;

    let tmp_path = path.with_extension("toml.tmp");
    let mut file = fs::File::create(&tmp_path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    drop(file);

    fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// Migrate a raw TOML value to the current schema version.
pub fn migrate(mut raw: toml::Value) -> toml::Value {
    let version = raw
        .get("schema_version")
        .and_then(|v| v.as_integer())
        .unwrap_or(0) as u32;

    match version {
        0 => {
            if let Some(table) = raw.as_table_mut() {
                table.insert(
                    "schema_version".into(),
                    toml::Value::Integer(i64::from(CURRENT_SCHEMA_VERSION)),
                );
            }
            raw
        }
        1 => raw,
        _ => {
            warn!("unknown schema_version {version}, treating as current");
            raw
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrips() {
        let config = Config::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(deserialized.camera.resolution_width, 1280);
        assert_eq!(deserialized.camera.resolution_height, 720);
        assert_eq!(deserialized.camera.fps, 30);
        assert!((deserialized.image.brightness - 0.0).abs() < f64::EPSILON);
        assert!((deserialized.image.contrast - 1.0).abs() < f64::EPSILON);
        assert!((deserialized.image.saturation - 1.0).abs() < f64::EPSILON);
        assert_eq!(deserialized.image.flip, FlipMethod::None);
        assert!(!deserialized.virtual_camera.enabled);
    }

    #[test]
    fn migrate_versionless_to_v1() {
        let raw: toml::Value = toml::from_str(
            r#"
            [camera]
            selected_id = "test-cam"

            [image]
            brightness = 0.5
        "#,
        )
        .unwrap();

        let migrated = migrate(raw);
        assert_eq!(
            migrated.get("schema_version").and_then(|v| v.as_integer()),
            Some(1)
        );
        assert_eq!(
            migrated
                .get("camera")
                .and_then(|c| c.get("selected_id"))
                .and_then(|v| v.as_str()),
            Some("test-cam")
        );
    }

    #[test]
    fn migrate_v1_is_noop() {
        let raw: toml::Value = toml::from_str("schema_version = 1\n").unwrap();
        let migrated = migrate(raw.clone());
        assert_eq!(raw, migrated);
    }

    #[test]
    fn deserialize_partial_config() {
        let config: Config = toml::from_str("schema_version = 1\n").unwrap();
        assert_eq!(config.camera.resolution_width, 1280);
        assert!((config.image.contrast - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn flip_method_serde_roundtrip() {
        let img = ImageSettings {
            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            flip: FlipMethod::HorizontalFlip,
        };
        let s = toml::to_string(&img).unwrap();
        assert!(s.contains("horizontal-flip"));
        let parsed: ImageSettings = toml::from_str(&s).unwrap();
        assert_eq!(parsed.flip, FlipMethod::HorizontalFlip);
    }
}
