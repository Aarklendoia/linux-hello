//! Détecteur de visages SCRFD-500M via tract-onnx
//!
//! SCRFD (Sample and Computation Redistribution for Efficient Face Detection)
//! modèle 500M depuis insightface ONNX zoo.
//! Input : RGB 640×640 normalisé (mean=[127.5, 127.5, 127.5], std=128)
//! Output : bounding boxes + scores + landmarks 5 points

use crate::{FaceDetector, FaceError, FaceRegion};
use std::path::Path;

#[cfg(feature = "tract")]
type TractPlan = tract_onnx::prelude::SimplePlan<
    tract_onnx::prelude::TypedFact,
    Box<dyn tract_onnx::prelude::TypedOp>,
    tract_onnx::prelude::Graph<
        tract_onnx::prelude::TypedFact,
        Box<dyn tract_onnx::prelude::TypedOp>,
    >,
>;

/// Détecteur SCRFD-500M basé sur tract-onnx
#[cfg(feature = "tract")]
pub struct ScrfdDetector {
    model: TractPlan,
    input_size: u32,
    confidence_threshold: f32,
    nms_threshold: f32,
}

#[cfg(feature = "tract")]
impl ScrfdDetector {
    /// Charger le modèle SCRFD depuis un fichier .onnx
    pub fn load(model_path: &Path) -> Result<Self, FaceError> {
        use tract_onnx::prelude::*;

        let model = tract_onnx::onnx()
            .model_for_path(model_path)
            .map_err(|e| FaceError::ModelLoadError(format!("SCRFD: {}", e)))?
            .with_input_fact(
                0,
                InferenceFact::dt_shape(f32::datum_type(), tvec![1, 3, 640, 640]),
            )
            .map_err(|e| FaceError::ModelLoadError(format!("SCRFD input: {}", e)))?
            .into_optimized()
            .map_err(|e| FaceError::ModelLoadError(format!("SCRFD optimize: {}", e)))?
            .into_runnable()
            .map_err(|e| FaceError::ModelLoadError(format!("SCRFD runnable: {}", e)))?;

        tracing::info!("Modèle SCRFD-500M chargé: {}", model_path.display());

        Ok(Self {
            model,
            input_size: 640,
            confidence_threshold: 0.5,
            nms_threshold: 0.4,
        })
    }

    /// Redimensionner une image RGB en 640×640 (letterbox)
    fn letterbox_rgb(&self, data: &[u8], src_w: u32, src_h: u32) -> (Vec<f32>, f32, f32) {
        let dst = self.input_size as usize;

        // Facteur de redimensionnement uniforme (preserve ratio)
        let scale = (dst as f32 / src_w as f32).min(dst as f32 / src_h as f32);
        let new_w = (src_w as f32 * scale).round() as usize;
        let new_h = (src_h as f32 * scale).round() as usize;
        let pad_x = (dst - new_w) / 2;
        let pad_y = (dst - new_h) / 2;

        // Fond gris (127.5 → normalisé à 0.0)
        let mut tensor = vec![0.0f32; 3 * dst * dst];

        for dy in 0..new_h {
            let sy = ((dy as f32 / scale) as usize).min(src_h as usize - 1);
            for dx in 0..new_w {
                let sx = ((dx as f32 / scale) as usize).min(src_w as usize - 1);
                let src_idx = (sy * src_w as usize + sx) * 3;
                let dst_y = pad_y + dy;
                let dst_x = pad_x + dx;
                // CHW order, normalisé : (pixel - 127.5) / 128.0
                for c in 0..3 {
                    tensor[c * dst * dst + dst_y * dst + dst_x] =
                        (data[src_idx + c] as f32 - 127.5) / 128.0;
                }
            }
        }

        (tensor, scale, pad_x as f32)
    }

    /// NMS (Non-Maximum Suppression)
    fn nms(&self, mut boxes: Vec<FaceRegion>) -> Vec<FaceRegion> {
        boxes.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        let mut keep = Vec::new();

        while !boxes.is_empty() {
            let best = boxes.remove(0);
            boxes.retain(|b| iou(&best.bounding_box, &b.bounding_box) < self.nms_threshold);
            keep.push(best);
        }
        keep
    }
}

/// IoU (Intersection over Union) pour deux bounding boxes (x, y, w, h)
fn iou(a: &(u32, u32, u32, u32), b: &(u32, u32, u32, u32)) -> f32 {
    let ax2 = a.0 + a.2;
    let ay2 = a.1 + a.3;
    let bx2 = b.0 + b.2;
    let by2 = b.1 + b.3;

    let ix1 = a.0.max(b.0);
    let iy1 = a.1.max(b.1);
    let ix2 = ax2.min(bx2);
    let iy2 = ay2.min(by2);

    if ix2 <= ix1 || iy2 <= iy1 {
        return 0.0;
    }

    let inter = ((ix2 - ix1) * (iy2 - iy1)) as f32;
    let area_a = (a.2 * a.3) as f32;
    let area_b = (b.2 * b.3) as f32;
    inter / (area_a + area_b - inter).max(1e-6)
}

#[cfg(feature = "tract")]
impl FaceDetector for ScrfdDetector {
    fn detect(
        &self,
        frame_data: &[u8],
        width: u32,
        height: u32,
        channels: u32,
    ) -> Result<Vec<FaceRegion>, FaceError> {
        use tract_onnx::prelude::*;

        if frame_data.is_empty() || channels != 3 {
            return Ok(vec![]);
        }

        let (tensor_data, scale, pad_x) = self.letterbox_rgb(frame_data, width, height);
        let dst = self.input_size as usize;

        let input_array = tract_ndarray::Array4::from_shape_vec((1, 3, dst, dst), tensor_data)
            .map_err(|e| FaceError::DetectionFailed(e.to_string()))?;
        let input: Tensor = input_array.into();

        let outputs = self
            .model
            .run(tvec![input.into()])
            .map_err(|e| FaceError::DetectionFailed(e.to_string()))?;

        // Le modèle SCRFD-500M (insightface) produit 3 sorties :
        //   [0] scores  : (1, N)      — confiance par boîte
        //   [1] boxes   : (1, N, 4)   — x1y1x2y2 normalisés sur 640
        //   [2] kpoints : (1, N, 10)  — landmarks 5pts (optionnel)
        let scores = outputs[0]
            .to_array_view::<f32>()
            .map_err(|e| FaceError::DetectionFailed(e.to_string()))?;
        let bboxes = outputs[1]
            .to_array_view::<f32>()
            .map_err(|e| FaceError::DetectionFailed(e.to_string()))?;

        let n = scores.shape()[1];
        let mut regions = Vec::new();
        let pad_y = (dst as f32 - (height as f32 * scale).round()) / 2.0;

        for i in 0..n {
            let conf = scores[[0, i]];
            if conf < self.confidence_threshold {
                continue;
            }

            // Coordonnées dans l'espace letterbox 640×640
            let x1 = bboxes[[0, i, 0]];
            let y1 = bboxes[[0, i, 1]];
            let x2 = bboxes[[0, i, 2]];
            let y2 = bboxes[[0, i, 3]];

            // Retransformer vers espace image original
            let ox1 = ((x1 - pad_x) / scale).max(0.0) as u32;
            let oy1 = ((y1 - pad_y) / scale).max(0.0) as u32;
            let ox2 = ((x2 - pad_x) / scale).min(width as f32) as u32;
            let oy2 = ((y2 - pad_y) / scale).min(height as f32) as u32;

            if ox2 <= ox1 || oy2 <= oy1 {
                continue;
            }

            // Landmarks (5 points × 2 coords) si la sortie est disponible
            let landmarks = if outputs.len() >= 3 {
                if let Ok(kp) = outputs[2].to_array_view::<f32>() {
                    (0..5)
                        .filter_map(|k| {
                            if kp.shape().len() >= 3 && i < kp.shape()[1] {
                                let kx = ((kp[[0, i, k * 2]] - pad_x) / scale).max(0.0);
                                let ky = ((kp[[0, i, k * 2 + 1]] - pad_y) / scale).max(0.0);
                                Some((kx, ky))
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            regions.push(FaceRegion {
                bounding_box: (ox1, oy1, ox2 - ox1, oy2 - oy1),
                confidence: conf,
                landmarks,
            });
        }

        let regions = self.nms(regions);
        tracing::debug!("SCRFD: {} visages détectés", regions.len());
        Ok(regions)
    }

    fn name(&self) -> &str {
        "SCRFD-500M"
    }

    fn model_version(&self) -> &str {
        "insightface-onnx-zoo"
    }
}

/// Détecteur stub utilisé en fallback si le modèle ONNX n'est pas disponible
pub struct ScrfdFallback;

impl FaceDetector for ScrfdFallback {
    fn detect(
        &self,
        frame_data: &[u8],
        width: u32,
        height: u32,
        _channels: u32,
    ) -> Result<Vec<FaceRegion>, FaceError> {
        if frame_data.is_empty() || width == 0 || height == 0 {
            return Ok(vec![]);
        }
        // Stub : retourner un visage centré avec confiance fixe
        let bx = width / 4;
        let by = height / 4;
        Ok(vec![FaceRegion {
            bounding_box: (bx, by, width / 2, height / 2),
            confidence: 0.70,
            landmarks: vec![
                ((width / 3) as f32, (height / 3) as f32),
                ((2 * width / 3) as f32, (height / 3) as f32),
                ((width / 2) as f32, (height / 2) as f32),
                ((width / 3) as f32, (2 * height / 3) as f32),
                ((2 * width / 3) as f32, (2 * height / 3) as f32),
            ],
        }])
    }

    fn name(&self) -> &str {
        "SCRFD-Fallback"
    }

    fn model_version(&self) -> &str {
        "stub-0.1"
    }
}
