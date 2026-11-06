/// Errors returned by the AI crate.
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    /// ONNX Runtime error (only with the `ort` feature).
    #[cfg(feature = "ort")]
    #[error("ort error: {0}")]
    Ort(#[from] ort::Error),

    /// The input frame had unexpected dimensions or format.
    #[error("invalid input: {0}")]
    InvalidInput(String),
}
