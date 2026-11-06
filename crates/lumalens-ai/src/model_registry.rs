use std::path::PathBuf;

/// Metadata for a downloadable segmentation model.
pub struct ModelInfo {
    /// Short unique identifier.
    pub id: &'static str,
    /// Human-readable name for the UI.
    pub display_name: &'static str,
    /// Download URL for the ONNX file.
    pub url: &'static str,
    /// Expected SHA-256 hex digest.
    pub sha256: &'static str,
    /// File size in bytes.
    pub size_bytes: u64,
    /// SPDX license identifier.
    pub license: &'static str,
}

static MODELS: &[ModelInfo] = &[ModelInfo {
    id: "selfie-segmentation",
    display_name: "Selfie Segmentation (MobileNetV3)",
    url: "https://huggingface.co/onnx-community/selfie_segmentation/resolve/main/onnx/model.onnx",
    sha256: "3241ac4ad8aa35bdaf33946776db29f7c283a413aa0b0dacb9483594b4531aad",
    size_bytes: 462_688,
    license: "Apache-2.0",
}];

/// Returns all known segmentation models.
pub fn available_models() -> &'static [ModelInfo] {
    MODELS
}

/// Returns `true` if the model file exists in the XDG data directory.
pub fn is_downloaded(info: &ModelInfo) -> bool {
    model_path(info).is_file()
}

/// Returns the expected on-disk path for a model.
pub(crate) fn model_path(info: &ModelInfo) -> PathBuf {
    let data_dir = dirs_data_dir().join("lumalens").join("models");
    data_dir.join(format!("{}.onnx", info.id))
}

fn dirs_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").expect("HOME not set");
            PathBuf::from(home).join(".local/share")
        })
}
