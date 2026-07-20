//! Test-only fakes for `FaceDetector`/`EmbeddingExtractor`.
//!
//! Lets `CameraManager`/`FaceAuthDaemon` orchestration (retry loops,
//! embedding averaging, quality scoring, the consecutive-match/liveness
//! gate) be tested deterministically, without real hardware, ONNX models,
//! or the ORT-dylib-load hazard documented in `hello_face_core::lib`'s
//! `ensure_ort_dylib_path` (`create_detector`/`create_extractor` can hang
//! rather than fail fast if the model file is present but the dylib isn't —
//! see `hello_face_core::scrfd_detector`'s test comments for the historical
//! bug this caused). Only ever compiled for `cargo test`, never shipped.
#![cfg(test)]

use hello_face_core::{
    Embedding, EmbeddingExtractor, EmbeddingMetadata, FaceDetector, FaceError, FaceRegion,
};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fixed bounding box roughly centered in a `w`x`h` frame — good enough
/// for tests that don't care about the exact region, just that a face was
/// "found".
pub(crate) fn default_face_region(w: u32, h: u32) -> FaceRegion {
    FaceRegion {
        bounding_box: (w / 4, h / 4, w / 2, h / 2),
        confidence: 0.99,
        landmarks: vec![],
    }
}

/// An all-zero RGB888 buffer of the given dimensions — matches the shape of
/// `CameraManager::capture_frames`'s own stub-frame fallback.
pub(crate) fn blank_rgb_frame(w: u32, h: u32) -> Vec<u8> {
    vec![0u8; (w * h * 3) as usize]
}

/// A `FaceDetector` whose detection outcome is fully controlled by the test.
pub(crate) enum FakeDetector {
    /// Always reports a face at the given region.
    Always(FaceRegion),
    /// Never reports a face.
    Never,
    /// Reports a face on every other call (starting with a hit), for tests
    /// that need a mixed hit/miss sequence across several frames.
    Alternating {
        region: FaceRegion,
        calls: AtomicUsize,
    },
}

impl FakeDetector {
    pub(crate) fn always_detects(region: FaceRegion) -> Self {
        Self::Always(region)
    }

    pub(crate) fn never_detects() -> Self {
        Self::Never
    }

    pub(crate) fn alternating(region: FaceRegion) -> Self {
        Self::Alternating {
            region,
            calls: AtomicUsize::new(0),
        }
    }
}

impl FaceDetector for FakeDetector {
    fn detect(
        &self,
        _frame_data: &[u8],
        _width: u32,
        _height: u32,
        _channels: u32,
    ) -> Result<Vec<FaceRegion>, FaceError> {
        match self {
            Self::Always(region) => Ok(vec![region.clone()]),
            Self::Never => Ok(vec![]),
            Self::Alternating { region, calls } => {
                let n = calls.fetch_add(1, Ordering::SeqCst);
                if n % 2 == 0 {
                    Ok(vec![region.clone()])
                } else {
                    Ok(vec![])
                }
            }
        }
    }

    fn name(&self) -> &str {
        "fake-detector"
    }

    fn model_version(&self) -> &str {
        "test"
    }
}

/// An `EmbeddingExtractor` that returns a fixed or cycling sequence of
/// embedding vectors, regardless of the actual pixel data — lets a test
/// control exactly which embedding a "captured" frame produces.
pub(crate) enum FakeExtractor {
    /// Always returns the same vector.
    Fixed { vector: Vec<f32>, quality: f32 },
    /// Cycles through `vectors` in order (wrapping), one per call — used to
    /// prove real multi-frame averaging, since a single fixed vector can
    /// only prove normalization.
    Sequence {
        vectors: Vec<Vec<f32>>,
        quality: f32,
        calls: AtomicUsize,
    },
}

impl FakeExtractor {
    pub(crate) fn with_vector(vector: Vec<f32>, quality: f32) -> Self {
        Self::Fixed { vector, quality }
    }

    pub(crate) fn sequence(vectors: Vec<Vec<f32>>, quality: f32) -> Self {
        assert!(!vectors.is_empty(), "sequence needs at least one vector");
        Self::Sequence {
            vectors,
            quality,
            calls: AtomicUsize::new(0),
        }
    }
}

impl EmbeddingExtractor for FakeExtractor {
    fn extract(
        &self,
        _face_region: &FaceRegion,
        _frame_data: &[u8],
        _width: u32,
        _height: u32,
        _channels: u32,
    ) -> Result<Embedding, FaceError> {
        let (vector, quality) = match self {
            Self::Fixed { vector, quality } => (vector.clone(), *quality),
            Self::Sequence {
                vectors,
                quality,
                calls,
            } => {
                let n = calls.fetch_add(1, Ordering::SeqCst);
                (vectors[n % vectors.len()].clone(), *quality)
            }
        };
        Ok(Embedding {
            vector,
            metadata: EmbeddingMetadata {
                model: "fake".to_string(),
                model_version: "test".to_string(),
                extracted_at: 0,
                quality_score: quality,
            },
        })
    }

    fn model_name(&self) -> &str {
        "fake-extractor"
    }

    fn model_version(&self) -> &str {
        "test"
    }

    fn embedding_dimension(&self) -> usize {
        match self {
            Self::Fixed { vector, .. } => vector.len(),
            Self::Sequence { vectors, .. } => vectors.first().map(Vec::len).unwrap_or(0),
        }
    }
}
