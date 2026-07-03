//! ArcFace MobileNetV3 embedding extractor via tract-onnx
//!
//! Model  : w600k_mbf.onnx (ArcFace trained on WebFace600K)
//! Input  : RGB 112x112, normalized mean=127.5/std=128.0, CHW format
//! Output : L2-normalized 512-dim vector

use crate::{Embedding, EmbeddingExtractor, EmbeddingMetadata, FaceError, FaceRegion};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// tract 0.23: into_runnable() returns Arc<SimplePlan<...>>, run() is on Arc
#[cfg(feature = "tract")]
type TractPlan = std::sync::Arc<tract_onnx::prelude::TypedRunnableModel>;

/// ArcFace MobileNetV3 extractor (w600k_mbf)
#[cfg(feature = "tract")]
pub struct ArcFaceExtractor {
    model: TractPlan,
}

#[cfg(feature = "tract")]
impl ArcFaceExtractor {
    /// Load the ArcFace model from an .onnx file
    pub fn load(model_path: &Path) -> Result<Self, FaceError> {
        use tract_onnx::prelude::*;

        let model = tract_onnx::onnx()
            .model_for_path(model_path)
            .map_err(|e| FaceError::ModelLoadError(format!("ArcFace: {}", e)))?
            .with_input_fact(
                0,
                InferenceFact::dt_shape(f32::datum_type(), tvec![1, 3, 112, 112]),
            )
            .map_err(|e| FaceError::ModelLoadError(format!("ArcFace input: {}", e)))?
            .into_optimized()
            .map_err(|e| FaceError::ModelLoadError(format!("ArcFace optimize: {}", e)))?
            .into_runnable()
            .map_err(|e| FaceError::ModelLoadError(format!("ArcFace runnable: {}", e)))?;

        tracing::info!(
            "ArcFace model (w600k_mbf) loaded: {}",
            model_path.display()
        );
        Ok(Self { model })
    }

    /// Align and crop the face into a 112x112 RGB patch
    ///
    /// Uses the SCRFD bounding box. If landmarks are available,
    /// an affine transform (5 points) will eventually be applied.
    fn align_face(
        &self,
        face: &FaceRegion,
        frame_data: &[u8],
        frame_w: u32,
        frame_h: u32,
    ) -> Vec<f32> {
        const SIZE: usize = 112;
        let (bx, by, bw, bh) = face.bounding_box;

        // Crop with a 20% margin
        let margin_x = (bw as f32 * 0.1) as u32;
        let margin_y = (bh as f32 * 0.1) as u32;
        let x1 = bx.saturating_sub(margin_x);
        let y1 = by.saturating_sub(margin_y);
        let x2 = (bx + bw + margin_x).min(frame_w);
        let y2 = (by + bh + margin_y).min(frame_h);
        let crop_w = (x2 - x1).max(1);
        let crop_h = (y2 - y1).max(1);

        // Resize the crop to 112x112 and normalize into CHW
        let mut tensor = vec![0.0f32; 3 * SIZE * SIZE];

        for dy in 0..SIZE {
            let sy = (dy as f32 * crop_h as f32 / SIZE as f32) as u32;
            for dx in 0..SIZE {
                let sx = (dx as f32 * crop_w as f32 / SIZE as f32) as u32;
                let px = (x1 + sx).min(frame_w - 1);
                let py = (y1 + sy).min(frame_h - 1);
                let src_idx = ((py * frame_w + px) * 3) as usize;

                if src_idx + 2 < frame_data.len() {
                    for c in 0..3usize {
                        tensor[c * SIZE * SIZE + dy * SIZE + dx] =
                            (frame_data[src_idx + c] as f32 - 127.5) / 128.0;
                    }
                }
            }
        }

        tensor
    }
}

#[cfg(feature = "tract")]
impl EmbeddingExtractor for ArcFaceExtractor {
    fn extract(
        &self,
        face_region: &FaceRegion,
        frame_data: &[u8],
        width: u32,
        height: u32,
        channels: u32,
    ) -> Result<Embedding, FaceError> {
        use tract_onnx::prelude::*;

        if channels != 3 || frame_data.is_empty() {
            return Err(FaceError::InvalidFrame("Expected RGB 3 channels".to_string()));
        }

        let aligned = self.align_face(face_region, frame_data, width, height);

        let input_array = tract_ndarray::Array4::from_shape_vec((1, 3, 112, 112), aligned)
            .map_err(|e| FaceError::ExtractionFailed(e.to_string()))?;
        let input: Tensor = input_array.into();

        let outputs = self
            .model
            .run(tvec![input.into()])
            .map_err(|e| FaceError::ExtractionFailed(e.to_string()))?;

        let raw = outputs[0]
            .to_plain_array_view::<f32>()
            .map_err(|e| FaceError::ExtractionFailed(e.to_string()))?;

        let mut vector: Vec<f32> = raw.iter().copied().collect();

        // L2 normalization (the model may return unnormalized vectors)
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
        for v in &mut vector {
            *v /= norm;
        }

        // Quality score: norm of the vector before normalization
        // (the higher it is, the sharper / better-framed the face)
        let quality = (norm / 20.0).clamp(0.0, 1.0);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        tracing::debug!(
            "ArcFace: 512-dim embedding extracted, quality={:.3}, confidence={:.3}",
            quality,
            face_region.confidence
        );

        Ok(Embedding {
            vector,
            metadata: EmbeddingMetadata {
                model: "arcface-w600k-mbf".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: now,
                quality_score: quality * face_region.confidence,
            },
        })
    }

    fn model_name(&self) -> &str {
        "arcface-w600k-mbf"
    }

    fn model_version(&self) -> &str {
        "insightface-onnx-zoo"
    }

    fn embedding_dimension(&self) -> usize {
        512
    }
}

/// Stub extractor used as a fallback if the ONNX model is not available
pub struct ArcFaceFallback;

impl EmbeddingExtractor for ArcFaceFallback {
    fn extract(
        &self,
        face_region: &FaceRegion,
        frame_data: &[u8],
        width: u32,
        height: u32,
        _channels: u32,
    ) -> Result<Embedding, FaceError> {
        // Reproducible embedding based on the face crop pixels
        let (bx, by, bw, bh) = face_region.bounding_box;
        let mut vector = vec![0.0f32; 512];

        for dy in 0..bh.min(height - by) {
            for dx in 0..bw.min(width - bx) {
                let idx = (((by + dy) * width + (bx + dx)) * 3) as usize;
                if idx + 2 < frame_data.len() {
                    let dim = ((dy * bw + dx) % 512) as usize;
                    vector[dim] += (frame_data[idx] as f32
                        + frame_data[idx + 1] as f32
                        + frame_data[idx + 2] as f32)
                        / (3.0 * 255.0);
                }
            }
        }

        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
        for v in &mut vector {
            *v /= norm;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Embedding {
            vector,
            metadata: EmbeddingMetadata {
                model: "arcface-fallback".to_string(),
                model_version: "stub-0.1".to_string(),
                extracted_at: now,
                quality_score: 0.6 * face_region.confidence,
            },
        })
    }

    fn model_name(&self) -> &str {
        "arcface-fallback"
    }

    fn model_version(&self) -> &str {
        "stub-0.1"
    }

    fn embedding_dimension(&self) -> usize {
        512
    }
}
