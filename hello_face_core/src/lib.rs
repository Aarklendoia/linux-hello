//! Face recognition engine - Core abstraction and types
//!
//! This crate exposes a generic, backend-agnostic API for:
//! - Face detection in frames
//! - Embedding extraction (facial signatures)
//! - Embedding comparison and similarity scoring

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod liveness;
pub mod stub_detector;

#[cfg(feature = "tract")]
pub mod scrfd_detector;

#[cfg(feature = "tract")]
pub mod arcface_extractor;

/// Face detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceRegion {
    /// Bounding box: (x, y, width, height) in pixels
    pub bounding_box: (u32, u32, u32, u32),

    /// Confidence in this detection (0.0 to 1.0)
    pub confidence: f32,

    /// Optional landmarks: position of eyes, nose, mouth, etc.
    /// Format: [(x, y), ...] in pixels relative to the region
    pub landmarks: Vec<(f32, f32)>,
}

/// Embedding (facial fingerprint) - high-dimensional vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// Vector of float values (typically 128, 256 or 512 dims)
    pub vector: Vec<f32>,

    /// Extraction metadata
    pub metadata: EmbeddingMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingMetadata {
    /// Model used for extraction (e.g. "arcface_mobilenet", "facenet")
    pub model: String,

    /// Model version
    pub model_version: String,

    /// Extraction timestamp (Unix timestamp)
    pub extracted_at: u64,

    /// Estimated extraction quality (0.0-1.0)
    pub quality_score: f32,
}

/// Possible errors from the recognition engine
#[derive(Debug, Error)]
pub enum FaceError {
    #[error("Model loading failed: {0}")]
    ModelLoadError(String),

    #[error("Detection failed: {0}")]
    DetectionFailed(String),

    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("Invalid frame: {0}")]
    InvalidFrame(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Trait for face detection implementations
pub trait FaceDetector: Send + Sync {
    /// Detect faces in an RGB/grayscale frame
    ///
    /// # Arguments
    /// * `frame_data` - raw pixel data
    /// * `width` - width in pixels
    /// * `height` - height in pixels
    /// * `channels` - number of channels (1 for grayscale, 3 for RGB)
    ///
    /// # Returns
    /// Vector of detected face regions
    fn detect(
        &self,
        frame_data: &[u8],
        width: u32,
        height: u32,
        channels: u32,
    ) -> Result<Vec<FaceRegion>, FaceError>;

    /// Detector name (e.g. "retinaface", "yolov8-face")
    fn name(&self) -> &str;

    /// Version of the model used
    fn model_version(&self) -> &str;
}

/// Trait for embedding extraction implementations
pub trait EmbeddingExtractor: Send + Sync {
    /// Extract an embedding from a face region
    ///
    /// # Arguments
    /// * `face_region` - detected face region
    /// * `frame_data` - raw frame data
    /// * `width` - frame width
    /// * `height` - frame height
    /// * `channels` - channels
    ///
    /// # Returns
    /// High-dimensional embedding
    fn extract(
        &self,
        face_region: &FaceRegion,
        frame_data: &[u8],
        width: u32,
        height: u32,
        channels: u32,
    ) -> Result<Embedding, FaceError>;

    /// Model name (e.g. "arcface", "facenet")
    fn model_name(&self) -> &str;

    /// Version
    fn model_version(&self) -> &str;

    /// Expected vector dimension
    fn embedding_dimension(&self) -> usize;
}

/// Simple histogram-based implementation for prototyping
pub mod simple_implementation {
    use super::*;
    use std::time::SystemTime;

    /// Simple histogram-based embedding extractor
    pub struct SimpleEmbedder;

    impl EmbeddingExtractor for SimpleEmbedder {
        fn extract(
            &self,
            face_region: &FaceRegion,
            frame_data: &[u8],
            width: u32,
            height: u32,
            channels: u32,
        ) -> Result<Embedding, FaceError> {
            let (x, y, w, h) = face_region.bounding_box;

            // Extract the histogram from the face region
            let mut hist_r = [0u32; 32];
            let mut hist_g = [0u32; 32];
            let mut hist_b = [0u32; 32];

            for py in y..std::cmp::min(y + h, height) {
                for px in x..std::cmp::min(x + w, width) {
                    let idx = ((py * width + px) * channels) as usize;
                    if idx + 2 < frame_data.len() {
                        let r = frame_data[idx];
                        let g = frame_data[idx + 1];
                        let b = frame_data[idx + 2];

                        hist_r[(r as usize) >> 3] += 1;
                        hist_g[(g as usize) >> 3] += 1;
                        hist_b[(b as usize) >> 3] += 1;
                    }
                }
            }

            // Normalize the histogram into a vector
            let total = (w * h) as f32;
            let mut vector = Vec::with_capacity(96);

            for h_val in hist_r.iter() {
                vector.push(*h_val as f32 / total);
            }
            for h_val in hist_g.iter() {
                vector.push(*h_val as f32 / total);
            }
            for h_val in hist_b.iter() {
                vector.push(*h_val as f32 / total);
            }

            Ok(Embedding {
                vector,
                metadata: EmbeddingMetadata {
                    model: "histogram".to_string(),
                    model_version: "0.1".to_string(),
                    extracted_at: SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    quality_score: 0.7,
                },
            })
        }

        fn model_name(&self) -> &str {
            "SimpleHistogram"
        }

        fn model_version(&self) -> &str {
            "0.1"
        }

        fn embedding_dimension(&self) -> usize {
            96 // 32 bins per RGB channel
        }
    }
}

/// Default path of the models directory
pub fn default_models_dir() -> std::path::PathBuf {
    // 1. Runtime environment variable (test/development override)
    if let Ok(p) = std::env::var("LINUX_HELLO_MODELS_DIR") {
        return std::path::PathBuf::from(p);
    }
    // 2. System path (installed linux-hello-models package)
    let system_path = std::path::PathBuf::from("/usr/share/linux-hello/models");
    if system_path.exists() {
        return system_path.clone();
    }
    // 3. Path compiled in by build.rs (development/CI)
    if let Some(p) = option_env!("LINUX_HELLO_MODELS_DIR") {
        let path = std::path::PathBuf::from(p);
        if path.exists() {
            return path;
        }
    }
    // 4. $XDG_DATA_HOME, falling back to ~/.local/share (dirs::data_dir()'s
    //    own fallback chain)
    if let Some(data_dir) = dirs::data_dir() {
        return data_dir.join("linux-hello/models");
    }
    system_path
}

/// What `ensure_ort_dylib_path` should do about `ORT_DYLIB_PATH`.
#[cfg(feature = "tract")]
#[derive(Debug, PartialEq, Eq)]
enum OrtDylibPath {
    /// Already set to a path that exists on disk — leave it alone.
    Unchanged,
    /// Not set, or set to a path that doesn't exist — use this instead.
    Fallback(String),
    /// Not set, or set to a path that doesn't exist, and none of the
    /// standard candidates exist either.
    NotFound,
}

/// Decide what to do about `ORT_DYLIB_PATH`, given its current value (if
/// any) and a way to check whether a candidate path exists on disk.
///
/// An explicit value (e.g. from `hello-daemon.service`) is trusted only if
/// it actually points to a real file — a stale/wrong value (e.g. after an
/// onnxruntime version bump changes the `.so` suffix, or on a non-multiarch
/// layout) must not block the fallback probing below, since that's exactly
/// the case this whole mechanism exists to guard against.
///
/// The versioned `.1.23` candidates match Debian/Ubuntu's `libonnxruntime1.23`
/// package; the unversioned `libonnxruntime.so` candidates are the
/// version-agnostic fallback other distros need — e.g. Arch's `onnxruntime-cpu`
/// package ships `libonnxruntime.so.1.27.1` with a `libonnxruntime.so` symlink,
/// which the `.1.23`-only candidates would never match.
#[cfg(feature = "tract")]
fn resolve_ort_dylib_path(current: Option<&str>, exists: impl Fn(&str) -> bool) -> OrtDylibPath {
    if current.is_some_and(&exists) {
        return OrtDylibPath::Unchanged;
    }
    const CANDIDATES: &[&str] = &[
        "/usr/lib/x86_64-linux-gnu/libonnxruntime.so.1.23",
        "/usr/lib/aarch64-linux-gnu/libonnxruntime.so.1.23",
        "/usr/lib/libonnxruntime.so.1.23",
        "/usr/lib/x86_64-linux-gnu/libonnxruntime.so",
        "/usr/lib/aarch64-linux-gnu/libonnxruntime.so",
        "/usr/lib/libonnxruntime.so",
    ];
    match CANDIDATES.iter().find(|c| exists(c)) {
        Some(c) => OrtDylibPath::Fallback(c.to_string()),
        None => OrtDylibPath::NotFound,
    }
}

/// Ensures `ORT_DYLIB_PATH` is set to a real file before the first `ort` call.
///
/// `hello-daemon.service` sets it explicitly, but a developer running the
/// daemon/CLI/tests directly from a shell typically won't have it set. When
/// that happens, `ort`'s own dynamic-library lookup can hang indefinitely
/// instead of failing fast (observed: a full deadlock, not just slowness —
/// every runtime thread parked in `futex_wait`, never recovering). Since the
/// library is reliably at a fixed multiarch path when installed via apt
/// (`libonnxruntime1.23`) or the linux-hello .deb's own runtime dependency,
/// default to that instead of leaving it to `ort` to figure out.
#[cfg(feature = "tract")]
fn ensure_ort_dylib_path() {
    let current = std::env::var("ORT_DYLIB_PATH").ok();
    let exists = |p: &str| std::path::Path::new(p).exists();

    if let Some(val) = &current {
        if !exists(val) {
            tracing::warn!(
                "ORT_DYLIB_PATH is set to {:?} but that file doesn't exist; falling back to standard locations",
                val
            );
        }
    }

    match resolve_ort_dylib_path(current.as_deref(), exists) {
        OrtDylibPath::Unchanged => {}
        OrtDylibPath::Fallback(path) => {
            tracing::debug!("Defaulting ORT_DYLIB_PATH to {} (found on disk)", path);
            // SAFETY: called once per detector/extractor creation, at daemon
            // startup, before any other thread reads this process's env vars.
            unsafe {
                std::env::set_var("ORT_DYLIB_PATH", &path);
            }
        }
        OrtDylibPath::NotFound => {
            tracing::warn!(
                "ORT_DYLIB_PATH not usable and libonnxruntime.so.1.23 not found in standard \
                 locations; ONNX model loading may hang or fail. Set ORT_DYLIB_PATH explicitly \
                 to fix this."
            );
        }
    }
}

/// Creates the most capable face detector available.
///
/// If the ONNX model is present and the "tract" feature is enabled,
/// returns a `ScrfdDetector`. Otherwise, returns the stub fallback.
pub fn create_detector(models_dir: &std::path::Path) -> Box<dyn FaceDetector> {
    #[cfg(feature = "tract")]
    {
        ensure_ort_dylib_path();
        let model_path = models_dir.join("det_500m.onnx");
        if model_path.exists() {
            match scrfd_detector::ScrfdDetector::load(&model_path) {
                Ok(det) => {
                    tracing::info!("SCRFD-500M detector loaded from {:?}", model_path);
                    return Box::new(det);
                }
                Err(e) => {
                    tracing::warn!("SCRFD loading failed: {}, falling back to stub", e);
                }
            }
        } else {
            tracing::warn!(
                "SCRFD model missing: {:?}, falling back to stub",
                model_path
            );
        }
    }
    tracing::info!("Using stub detector (fallback)");
    scrfd_detector_fallback()
}

/// Creates the most capable embedding extractor available.
///
/// If the ONNX model is present and the "tract" feature is enabled,
/// returns an `ArcFaceExtractor`. Otherwise, returns the stub fallback.
pub fn create_extractor(models_dir: &std::path::Path) -> Box<dyn EmbeddingExtractor> {
    #[cfg(feature = "tract")]
    {
        ensure_ort_dylib_path();
        let model_path = models_dir.join("w600k_mbf.onnx");
        if model_path.exists() {
            match arcface_extractor::ArcFaceExtractor::load(&model_path) {
                Ok(ext) => {
                    tracing::info!("ArcFace extractor loaded from {:?}", model_path);
                    return Box::new(ext);
                }
                Err(e) => {
                    tracing::warn!("ArcFace loading failed: {}, falling back to stub", e);
                }
            }
        } else {
            tracing::warn!(
                "ArcFace model missing: {:?}, falling back to stub",
                model_path
            );
        }
    }
    tracing::info!("Using stub extractor (fallback)");
    arcface_extractor_fallback()
}

// Internal functions to instantiate fallbacks without the tract feature
fn scrfd_detector_fallback() -> Box<dyn FaceDetector> {
    #[cfg(feature = "tract")]
    {
        Box::new(scrfd_detector::ScrfdFallback)
    }
    #[cfg(not(feature = "tract"))]
    {
        Box::new(stub_detector::StubDetector::default())
    }
}

fn arcface_extractor_fallback() -> Box<dyn EmbeddingExtractor> {
    #[cfg(feature = "tract")]
    {
        Box::new(arcface_extractor::ArcFaceFallback)
    }
    #[cfg(not(feature = "tract"))]
    {
        Box::new(simple_implementation::SimpleEmbedder)
    }
}

#[cfg(test)]
mod factory_tests {
    use super::*;

    #[test]
    fn test_create_detector_fallback() {
        let tmp = std::path::Path::new("/tmp/nonexistent_models_dir_test");
        let det = create_detector(tmp);
        // Must return a valid detector even without models
        let frame = vec![128u8; 640 * 480 * 3];
        let result = det.detect(&frame, 640, 480, 3);
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_create_extractor_fallback() {
        let tmp = std::path::Path::new("/tmp/nonexistent_models_dir_test");
        let ext = create_extractor(tmp);
        assert!(ext.embedding_dimension() > 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "tract")]
    #[test]
    fn test_resolve_ort_dylib_path_keeps_an_existing_explicit_value() {
        let result = resolve_ort_dylib_path(Some("/some/real/path"), |p| p == "/some/real/path");
        assert_eq!(result, OrtDylibPath::Unchanged);
    }

    #[cfg(feature = "tract")]
    #[test]
    fn test_resolve_ort_dylib_path_falls_back_when_explicit_value_is_missing_on_disk() {
        // The systemd unit's hardcoded value points at a file that no longer
        // exists (e.g. after an onnxruntime version bump) — must not trust
        // it blindly, must probe the standard candidates instead.
        let result = resolve_ort_dylib_path(Some("/stale/path.so.1.23"), |p| {
            p == "/usr/lib/libonnxruntime.so.1.23"
        });
        assert_eq!(
            result,
            OrtDylibPath::Fallback("/usr/lib/libonnxruntime.so.1.23".to_string())
        );
    }

    #[cfg(feature = "tract")]
    #[test]
    fn test_resolve_ort_dylib_path_falls_back_when_unset() {
        let result = resolve_ort_dylib_path(None, |p| p == "/usr/lib/libonnxruntime.so.1.23");
        assert_eq!(
            result,
            OrtDylibPath::Fallback("/usr/lib/libonnxruntime.so.1.23".to_string())
        );
    }

    #[cfg(feature = "tract")]
    #[test]
    fn test_resolve_ort_dylib_path_tries_candidates_in_multiarch_then_generic_order() {
        // aarch64 exists but x86_64 doesn't — must pick aarch64, not fall
        // through to the generic /usr/lib candidate.
        let result = resolve_ort_dylib_path(None, |p| {
            p == "/usr/lib/aarch64-linux-gnu/libonnxruntime.so.1.23"
        });
        assert_eq!(
            result,
            OrtDylibPath::Fallback("/usr/lib/aarch64-linux-gnu/libonnxruntime.so.1.23".to_string())
        );
    }

    #[cfg(feature = "tract")]
    #[test]
    fn test_resolve_ort_dylib_path_falls_back_to_unversioned_so_on_non_debian_layouts() {
        // Arch's onnxruntime-cpu package ships libonnxruntime.so.1.27.1 with a
        // libonnxruntime.so symlink, not the Debian-versioned .1.23 filename —
        // none of the .1.23 candidates exist, only the unversioned one.
        let result = resolve_ort_dylib_path(None, |p| p == "/usr/lib/libonnxruntime.so");
        assert_eq!(
            result,
            OrtDylibPath::Fallback("/usr/lib/libonnxruntime.so".to_string())
        );
    }

    #[cfg(feature = "tract")]
    #[test]
    fn test_resolve_ort_dylib_path_not_found_when_nothing_exists() {
        assert_eq!(
            resolve_ort_dylib_path(None, |_| false),
            OrtDylibPath::NotFound
        );
        assert_eq!(
            resolve_ort_dylib_path(Some("/stale/path"), |_| false),
            OrtDylibPath::NotFound
        );
    }

    #[cfg(feature = "tract")]
    #[test]
    fn test_resolve_ort_dylib_path_treats_an_empty_explicit_value_as_unset() {
        // std::env::var returns Some("") for `ORT_DYLIB_PATH=""`, not None —
        // must not treat that as a valid explicit path.
        let result = resolve_ort_dylib_path(Some(""), |p| p == "/usr/lib/libonnxruntime.so.1.23");
        assert_eq!(
            result,
            OrtDylibPath::Fallback("/usr/lib/libonnxruntime.so.1.23".to_string())
        );
    }

    #[test]
    fn test_embedding_serialization() {
        let emb = Embedding {
            vector: vec![0.1, 0.2, 0.3],
            metadata: EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        };

        let json = serde_json::to_string(&emb).unwrap();
        let restored: Embedding = serde_json::from_str(&json).unwrap();
        assert_eq!(emb.vector, restored.vector);
    }

    #[test]
    fn test_default_models_dir_prefers_the_env_var_override() {
        // The env var is checked first, before any real filesystem path
        // (the system /usr/share/linux-hello/models install, XDG_DATA_HOME,
        // etc.), so this is safe to assert regardless of what's actually
        // installed on the machine running the test.
        //
        // SAFETY: no other test in this binary reads or writes
        // LINUX_HELLO_MODELS_DIR (grep-confirmed) — sequential save/restore
        // within this one test, so it can't race with itself either.
        let saved = std::env::var("LINUX_HELLO_MODELS_DIR").ok();

        unsafe {
            std::env::set_var("LINUX_HELLO_MODELS_DIR", "/scratch/override-models-dir");
        }
        assert_eq!(
            default_models_dir(),
            std::path::PathBuf::from("/scratch/override-models-dir")
        );

        unsafe {
            match &saved {
                Some(v) => std::env::set_var("LINUX_HELLO_MODELS_DIR", v),
                None => std::env::remove_var("LINUX_HELLO_MODELS_DIR"),
            }
        }
    }
}
