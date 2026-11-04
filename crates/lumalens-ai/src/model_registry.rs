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
    id: "mediapipe-selfie-small",
    display_name: "MediaPipe Selfie Segmentation (small)",
    url: "https://storage.googleapis.com/mediapipe-models/image_segmenter/selfie_segmenter/float16/latest/selfie_segmenter.tflite",
    sha256: "b0e6d012dbb949a63df87e5e0fea3e5a32e6b3e26abb64e256a4974d55546454",
    size_bytes: 262_112,
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
