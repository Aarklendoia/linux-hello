//! Face recognition engine - Core abstraction and types
//!
//! This crate exposes a generic, backend-agnostic API for:
//! - Face detection in frames
//! - Embedding extraction (facial signatures)
//! - Embedding comparison and similarity scoring

use serde::{Deserialize, Serialize};
use std::fmt;
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

/// Face verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchResult {
    /// Successful match
    Success {
        /// Similarity score (0.0 to 1.0 or Euclidean distance)
        score: f32,
        /// Method used
        method: String,
    },

    /// No face detected
    NoFace,

    /// Face detected but confidence insufficient
    LowConfidence { score: f32, required_threshold: f32 },

    /// Cancelled by the user
    AbortedByUser,

    /// Internal error
    InternalError(String),
}

impl fmt::Display for MatchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchResult::Success { score, method } => {
                write!(f, "Success ({}): score {:.2}", method, score)
            }
            MatchResult::NoFace => write!(f, "No face detected"),
            MatchResult::LowConfidence {
                score,
                required_threshold,
            } => {
                write!(
                    f,
                    "Insufficient confidence: {:.2} < {:.2}",
                    score, required_threshold
                )
            }
            MatchResult::AbortedByUser => write!(f, "Cancelled by user"),
            MatchResult::InternalError(e) => write!(f, "Internal error: {}", e),
        }
    }
}

/// Possible errors from the recognition engine
#[derive(Debug, Error)]
pub enum FaceError {
    #[error("Model loading failed: {0}")]
    ModelLoadError(String),

    #[error("Detection failed: {0}")]
    DetectionFailed(String),

    #[error("Embedding extraction failed: {0}")]
    EmbeddingFailed(String),

    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("Invalid frame: {0}")]
    InvalidFrame(String),

    #[error("No backend available")]
    NoBackendAvailable,

    #[error("Configuration error: {0}")]
    ConfigError(String),

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

/// Trait for similarity/distance computation
pub trait SimilarityMetric: Send + Sync {
    /// Compare two embeddings
    ///
    /// # Returns
    /// Score between 0.0 and 1.0 (or distance depending on the metric)
    /// Higher = more similar
    fn compare(&self, embedding1: &Embedding, embedding2: &Embedding) -> Result<f32, FaceError>;

    /// Metric name (e.g. "euclidean", "cosine")
    fn metric_name(&self) -> &str;
}

/// Verification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Required confidence threshold (0.0 to 1.0)
    pub confidence_threshold: f32,

    /// Required similarity threshold
    pub similarity_threshold: f32,

    /// Max timeout for detection (ms)
    pub detection_timeout_ms: u64,

    /// Number of allowed attempts
    pub max_attempts: u32,

    /// Context (login, sudo, screenlock, sddm)
    pub context: String,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.95,
            similarity_threshold: 0.6,
            detection_timeout_ms: 5000,
            max_attempts: 3,
            context: "default".to_string(),
        }
    }
}

/// Simple histogram-based implementation for prototyping
pub mod simple_implementation {
    use super::*;
    use std::time::SystemTime;

    /// Simple detector based on local contrast analysis
    pub struct SimpleDetector;

    impl FaceDetector for SimpleDetector {
        fn detect(
            &self,
            _frame_data: &[u8],
            width: u32,
            height: u32,
            _channels: u32,
        ) -> Result<Vec<FaceRegion>, FaceError> {
            // Simple heuristic: look for high-variance regions (facial features)
            // For prototyping, return a fixed central region (where faces usually are)
            // A real implementation would use Haar Cascade or YOLO

            let region_w = (width as f32 * 0.5) as u32;
            let region_h = (height as f32 * 0.6) as u32;
            let region_x = (width as f32 * 0.25) as u32;
            let region_y = (height as f32 * 0.2) as u32;

            Ok(vec![FaceRegion {
                bounding_box: (region_x, region_y, region_w, region_h),
                confidence: 0.8,
                landmarks: vec![],
            }])
        }

        fn name(&self) -> &str {
            "SimpleDetector"
        }

        fn model_version(&self) -> &str {
            "0.1"
        }
    }

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
    // 4. XDG_DATA_HOME
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(xdg).join("linux-hello/models");
    }
    // 5. HOME/.local/share
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".local/share/linux-hello/models");
    }
    system_path
}

/// Creates the most capable face detector available.
///
/// If the ONNX model is present and the "tract" feature is enabled,
/// returns a `ScrfdDetector`. Otherwise, returns the stub fallback.
pub fn create_detector(models_dir: &std::path::Path) -> Box<dyn FaceDetector> {
    #[cfg(feature = "tract")]
    {
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
    fn test_match_result_display() {
        let result = MatchResult::Success {
            score: 0.85,
            method: "cosine".to_string(),
        };
        assert!(result.to_string().contains("0.85"));
    }
}
