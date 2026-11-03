//! Image preset management.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use tracing::warn;

use crate::{ConfigError, ImageSettings};

const NATURAL_TOML: &str = include_str!("../resources/natural.toml");
const WARM_TOML: &str = include_str!("../resources/warm.toml");
const COOL_TOML: &str = include_str!("../resources/cool.toml");

const BUILTIN_PRESETS: &[(&str, &str)] = &[
    ("Natural", NATURAL_TOML),
    ("Warm", WARM_TOML),
    ("Cool", COOL_TOML),
];

/// A named image-settings preset.
#[derive(Debug, Clone)]
pub struct Preset {
    /// Display name (also the filename stem).
    pub name: String,
    /// Image adjustment values.
    pub image: ImageSettings,
}

fn profiles_dir() -> PathBuf {
    super::config_dir().join("profiles")
}

/// Copy built-in presets to the user's profiles directory if they don't exist.
pub fn ensure_builtin_presets() {
    let dir = profiles_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        warn!("failed to create profiles directory: {e}");
        return;
    }

    for (name, content) in BUILTIN_PRESETS {
        let path = dir.join(format!("{name}.toml"));
        if !path.exists() {
            if let Err(e) = fs::write(&path, content) {
                warn!("failed to write built-in preset {name}: {e}");
            }
        }
    }
}

/// List all presets from the user's profiles directory.
pub fn list_presets() -> Vec<Preset> {
    let dir = profiles_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut presets = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            match fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<ImageSettings>(&content) {
                    Ok(image) => presets.push(Preset { name, image }),
                    Err(e) => warn!("failed to parse preset {}: {e}", path.display()),
                },
                Err(e) => warn!("failed to read preset {}: {e}", path.display()),
            }
        }
    }

    presets.sort_by(|a, b| a.name.cmp(&b.name));
    presets
}

/// Save a preset to the user's profiles directory.
pub fn save_preset(name: &str, image: &ImageSettings) -> Result<(), ConfigError> {
    let dir = profiles_dir();
    fs::create_dir_all(&dir)?;

    let content = toml::to_string_pretty(image)?;
    let path = dir.join(format!("{name}.toml"));

    let tmp_path = path.with_extension("toml.tmp");
    let mut file = fs::File::create(&tmp_path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    drop(file);

    fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// Delete a preset from the user's profiles directory.
pub fn delete_preset(name: &str) -> Result<(), ConfigError> {
    let path = profiles_dir().join(format!("{name}.toml"));
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_presets_parse() {
        for (name, content) in BUILTIN_PRESETS {
            let image: ImageSettings = toml::from_str(content)
                .unwrap_or_else(|e| panic!("built-in preset {name} failed to parse: {e}"));
            assert!(
                image.brightness >= -1.0 && image.brightness <= 1.0,
                "{name}: brightness out of range"
            );
            assert!(
                image.contrast >= 0.0 && image.contrast <= 2.0,
                "{name}: contrast out of range"
            );
            assert!(
                image.saturation >= 0.0 && image.saturation <= 2.0,
                "{name}: saturation out of range"
            );
        }
    }

    #[test]
    fn image_settings_roundtrip() {
        let image = ImageSettings {
            brightness: 0.2,
            contrast: 1.5,
            saturation: 0.9,
            flip: lumalens_common::FlipMethod::HorizontalFlip,
        };
        let serialized = toml::to_string_pretty(&image).unwrap();
        let parsed: ImageSettings = toml::from_str(&serialized).unwrap();
        assert!((parsed.brightness - 0.2).abs() < f64::EPSILON);
        assert!((parsed.contrast - 1.5).abs() < f64::EPSILON);
        assert_eq!(parsed.flip, lumalens_common::FlipMethod::HorizontalFlip);
    }
}
