// Downloads the segmentation model at build time and caches it in
// `models/` next to this crate's Cargo.toml so subsequent builds are
// instant (no network). The file is then copied to OUT_DIR for
// `include_bytes!` to embed into the binary.

#[cfg(feature = "ort")]
fn main() {
    use std::path::PathBuf;

    const MODEL_ID: &str = "selfie-segmentation";
    const MODEL_URL: &str =
        "https://huggingface.co/onnx-community/selfie_segmentation/resolve/main/onnx/model.onnx";
    const MODEL_SHA256: &str = "3241ac4ad8aa35bdaf33946776db29f7c283a413aa0b0dacb9483594b4531aad";

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let cache_dir = manifest_dir.join("models");
    let cache_path = cache_dir.join(format!("{MODEL_ID}.onnx"));

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let out_path = out_dir.join(format!("{MODEL_ID}.onnx"));

    // If the cache is already valid, just copy to OUT_DIR and return.
    if cache_path.is_file() && verify_sha256(&cache_path, MODEL_SHA256) {
        std::fs::copy(&cache_path, &out_path).expect("failed to copy cached model to OUT_DIR");
        println!("cargo:rerun-if-changed=models/{MODEL_ID}.onnx");
        return;
    }

    // Download to cache.
    println!("cargo:warning=Downloading segmentation model (one-time, ~452 KB)…");
    std::fs::create_dir_all(&cache_dir).expect("failed to create models/ dir");

    let bytes = download(MODEL_URL);
    verify_or_panic(&bytes, MODEL_SHA256);

    std::fs::write(&cache_path, &bytes).expect("failed to write model cache");
    std::fs::write(&out_path, &bytes).expect("failed to write model to OUT_DIR");

    println!("cargo:rerun-if-changed=models/{MODEL_ID}.onnx");
}

#[cfg(not(feature = "ort"))]
fn main() {}

#[cfg(feature = "ort")]
fn download(url: &str) -> Vec<u8> {
    let response =
        reqwest::blocking::get(url).unwrap_or_else(|e| panic!("model download failed: {e}"));
    response
        .bytes()
        .unwrap_or_else(|e| panic!("model download failed reading body: {e}"))
        .to_vec()
}

#[cfg(feature = "ort")]
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

#[cfg(feature = "ort")]
fn verify_sha256(path: &std::path::Path, expected: &str) -> bool {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return false,
    };
    sha256_hex(&data) == expected
}

#[cfg(feature = "ort")]
fn verify_or_panic(bytes: &[u8], expected: &str) {
    let got = sha256_hex(bytes);
    assert_eq!(
        got, expected,
        "SHA-256 mismatch: expected {expected}, got {got}"
    );
}
