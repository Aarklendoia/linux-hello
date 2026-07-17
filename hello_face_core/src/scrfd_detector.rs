//! SCRFD-500M face detector via ONNX Runtime (ort)
//!
//! Input : RGB 640x640 CHW normalized (mean=127.5, std=128)
//! Output : 9 tensors (3 per stride 8/16/32: score, bbox, kps)

use crate::{FaceDetector, FaceError, FaceRegion};
use std::path::Path;

/// SCRFD-500M detector based on ONNX Runtime
#[cfg(feature = "tract")]
pub struct ScrfdDetector {
    session: std::sync::Mutex<ort::session::Session>,
    input_size: u32,
    confidence_threshold: f32,
    nms_threshold: f32,
}

#[cfg(feature = "tract")]
impl ScrfdDetector {
    pub fn load(model_path: &Path) -> Result<Self, FaceError> {
        let session = ort::session::Session::builder()
            .map_err(|e| FaceError::ModelLoadError(format!("ort builder: {}", e)))?
            .with_intra_threads(1)
            .map_err(|e| FaceError::ModelLoadError(format!("ort threads: {}", e)))?
            .commit_from_file(model_path)
            .map_err(|e| FaceError::ModelLoadError(format!("SCRFD load: {}", e)))?;

        tracing::info!("SCRFD-500M model loaded (ort): {}", model_path.display());

        Ok(Self {
            session: std::sync::Mutex::new(session),
            input_size: 640,
            confidence_threshold: 0.5,
            nms_threshold: 0.4,
        })
    }

    fn letterbox_rgb(&self, data: &[u8], src_w: u32, src_h: u32) -> (Vec<f32>, f32, f32, f32) {
        let dst = self.input_size as usize;
        let scale = (dst as f32 / src_w as f32).min(dst as f32 / src_h as f32);
        let new_w = (src_w as f32 * scale).round() as usize;
        let new_h = (src_h as f32 * scale).round() as usize;
        let pad_x = (dst - new_w) / 2;
        let pad_y = (dst - new_h) / 2;

        let mut tensor = vec![0.0f32; 3 * dst * dst];
        for dy in 0..new_h {
            let sy = ((dy as f32 / scale) as usize).min(src_h as usize - 1);
            for dx in 0..new_w {
                let sx = ((dx as f32 / scale) as usize).min(src_w as usize - 1);
                let src_idx = (sy * src_w as usize + sx) * 3;
                let dst_y = pad_y + dy;
                let dst_x = pad_x + dx;
                for c in 0..3 {
                    tensor[c * dst * dst + dst_y * dst + dst_x] =
                        (data[src_idx + c] as f32 - 127.5) / 128.0;
                }
            }
        }
        (tensor, scale, pad_x as f32, pad_y as f32)
    }

    fn nms(&self, mut boxes: Vec<FaceRegion>) -> Vec<FaceRegion> {
        boxes.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        let mut keep = Vec::new();
        while !boxes.is_empty() {
            let best = boxes.remove(0);
            boxes.retain(|b| iou(&best.bounding_box, &b.bounding_box) < self.nms_threshold);
            keep.push(best);
        }
        keep
    }

    fn generate_anchor_centers(stride: u32, input_size: u32) -> Vec<(f32, f32)> {
        let h = (input_size / stride) as usize;
        let w = (input_size / stride) as usize;
        let mut centers = Vec::with_capacity(h * w * 2);
        for row in 0..h {
            for col in 0..w {
                let cx = (col as u32 * stride) as f32;
                let cy = (row as u32 * stride) as f32;
                centers.push((cx, cy));
                centers.push((cx, cy));
            }
        }
        centers
    }
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
        if frame_data.is_empty() || width == 0 || height == 0 || channels != 3 {
            return Ok(vec![]);
        }

        // letterbox_rgb indexes frame_data assuming it holds exactly
        // width*height*channels bytes, with no bounds check of its own —
        // width/height come from the caller (ultimately the V4L2-negotiated
        // capture format), while frame_data is a separately-produced buffer
        // that can legitimately be shorter (e.g. a truncated read, or a
        // driver/format mismatch — see yuyv_to_rgb_strided's early `break`
        // on a short row in hello_camera). Reject rather than read past the
        // end, matching the same check StubDetector already does.
        let expected_size = (width as usize) * (height as usize) * (channels as usize);
        if frame_data.len() < expected_size {
            return Err(FaceError::InvalidFrame(format!(
                "Invalid frame size: {} < {}",
                frame_data.len(),
                expected_size
            )));
        }

        let (tensor_data, scale, pad_x, pad_y) = self.letterbox_rgb(frame_data, width, height);
        let dst = self.input_size as usize;

        let input = ndarray::Array4::<f32>::from_shape_vec((1, 3, dst, dst), tensor_data)
            .map_err(|e| FaceError::DetectionFailed(e.to_string()))?;

        let input_tensor = ort::value::Tensor::<f32>::from_array(input)
            .map_err(|e| FaceError::DetectionFailed(e.to_string()))?;

        tracing::debug!("SCRFD (ort): inference {}x{}", width, height);

        // All output processing happens within the lock's scope
        let regions_raw: Vec<FaceRegion> = {
            let mut session = self
                .session
                .lock()
                .map_err(|e| FaceError::DetectionFailed(format!("mutex: {}", e)))?;

            let outputs = session.run(ort::inputs![input_tensor]).map_err(|e| {
                tracing::error!("SCRFD ort.run() failed: {}", e);
                FaceError::DetectionFailed(e.to_string())
            })?;

            let n_outputs = outputs.len();
            tracing::debug!("SCRFD: {} outputs", n_outputs);

            // The SCRFD model groups its outputs by TYPE (not by stride):
            //   [0,1,2] = score_8, score_16, score_32 -> shape (N, 1) or (N,)
            //   [3,4,5] = bbox_8,  bbox_16,  bbox_32  -> shape (N, 4)
            //   [6,7,8] = kps_8,   kps_16,   kps_32   -> shape (N, 10)  (if present)
            let has_kps = n_outputs >= 9;
            let strides = [8u32, 16, 32];

            let mut regions = Vec::new();

            for (s_idx, &stride) in strides.iter().enumerate() {
                let score_idx = s_idx; // 0, 1, 2
                let bbox_idx = 3 + s_idx; // 3, 4, 5

                if bbox_idx >= n_outputs {
                    break;
                }

                let scores_view = outputs[score_idx]
                    .try_extract_array::<f32>()
                    .map_err(|e| FaceError::DetectionFailed(format!("score extract: {}", e)))?;
                let bboxes_view = outputs[bbox_idx]
                    .try_extract_array::<f32>()
                    .map_err(|e| FaceError::DetectionFailed(format!("bbox extract: {}", e)))?;

                let scores = scores_view.to_owned();
                let bboxes = bboxes_view.to_owned();

                let kps = if has_kps {
                    let kps_idx = 6 + s_idx;
                    outputs[kps_idx]
                        .try_extract_array::<f32>()
                        .ok()
                        .map(|v| v.to_owned())
                } else {
                    None
                };

                // N = number of anchors: first axis of the tensor (N,1) or (N,4)
                let n = scores.shape()[0];
                let anchor_centers = Self::generate_anchor_centers(stride, self.input_size);
                let n_anchors = anchor_centers.len().min(n);

                tracing::debug!(
                    "SCRFD stride={} n={} scores.shape={:?} bboxes.shape={:?}",
                    stride,
                    n,
                    scores.shape(),
                    bboxes.shape()
                );

                for i in 0..n_anchors {
                    // Score: (N,1) or (N,) or (1,N,1) or (1,N)
                    let conf = match scores.ndim() {
                        1 => scores[[i]],
                        2 => scores[[i, 0_usize]],
                        3 => scores[[0_usize, i, 0_usize]],
                        _ => continue,
                    };

                    if !conf.is_finite() || conf < self.confidence_threshold {
                        continue;
                    }

                    let (cx, cy) = anchor_centers[i];

                    // BBox: (N,4) or (1,N,4) — distances x stride from the anchor center
                    let (x1, y1, x2, y2) = match bboxes.ndim() {
                        2 => (
                            cx - bboxes[[i, 0_usize]] * stride as f32,
                            cy - bboxes[[i, 1_usize]] * stride as f32,
                            cx + bboxes[[i, 2_usize]] * stride as f32,
                            cy + bboxes[[i, 3_usize]] * stride as f32,
                        ),
                        3 => (
                            cx - bboxes[[0_usize, i, 0_usize]] * stride as f32,
                            cy - bboxes[[0_usize, i, 1_usize]] * stride as f32,
                            cx + bboxes[[0_usize, i, 2_usize]] * stride as f32,
                            cy + bboxes[[0_usize, i, 3_usize]] * stride as f32,
                        ),
                        _ => continue,
                    };

                    let ox1 = ((x1 - pad_x) / scale).max(0.0) as u32;
                    let oy1 = ((y1 - pad_y) / scale).max(0.0) as u32;
                    let ox2 = ((x2 - pad_x) / scale).min(width as f32) as u32;
                    let oy2 = ((y2 - pad_y) / scale).min(height as f32) as u32;

                    if ox2 <= ox1 || oy2 <= oy1 {
                        continue;
                    }

                    let landmarks: Vec<(f32, f32)> = kps
                        .as_ref()
                        .filter(|k| k.shape()[0] > i)
                        .map(|k| {
                            (0..5usize)
                                .map(|p| {
                                    let (kx_raw, ky_raw) = match k.ndim() {
                                        2 => (k[[i, p * 2]], k[[i, p * 2 + 1]]),
                                        _ => (k[[0_usize, i, p * 2]], k[[0_usize, i, p * 2 + 1]]),
                                    };
                                    let kx = ((cx + kx_raw * stride as f32) - pad_x) / scale;
                                    let ky = ((cy + ky_raw * stride as f32) - pad_y) / scale;
                                    (kx.max(0.0), ky.max(0.0))
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    regions.push(FaceRegion {
                        bounding_box: (ox1, oy1, ox2 - ox1, oy2 - oy1),
                        confidence: conf,
                        landmarks,
                    });
                }
            }

            regions
        }; // lock released here

        let regions = self.nms(regions_raw);
        tracing::debug!(
            "SCRFD: {} faces (conf>{:.2})",
            regions.len(),
            self.confidence_threshold
        );
        Ok(regions)
    }

    fn name(&self) -> &str {
        "SCRFD-500M"
    }

    fn model_version(&self) -> &str {
        "insightface-onnx-zoo"
    }
}

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

#[cfg(all(test, feature = "tract"))]
mod tests {
    use super::*;

    #[test]
    fn detect_rejects_a_truncated_frame_buffer_instead_of_reading_out_of_bounds() {
        let model_path = crate::default_models_dir().join("det_500m.onnx");
        // Check for the file up front rather than letting `load` fail:
        // when the onnxruntime dylib itself isn't installed (e.g. CI with
        // LINUX_HELLO_NO_MODEL_DOWNLOAD=1), ort's dylib-not-found error path
        // deadlocks instead of returning an Err (ort 2.0.0-rc.12), so we
        // can't rely on `load` to fail fast here.
        if !model_path.exists() {
            eprintln!(
                "Skipping: SCRFD model not available at {}",
                model_path.display()
            );
            return;
        }
        let Ok(detector) = ScrfdDetector::load(&model_path) else {
            // Real model not available in this environment — nothing to
            // test here, don't fail the suite over an environment gap.
            eprintln!(
                "Skipping: SCRFD model not available at {}",
                model_path.display()
            );
            return;
        };

        // Declares a 640x480x3 frame but only actually supplies 10 bytes —
        // exactly the mismatch a truncated capture or a driver reporting a
        // larger format than it delivers would produce.
        let truncated = vec![128u8; 10];
        let result = detector.detect(&truncated, 640, 480, 3);
        assert!(
            result.is_err(),
            "must reject a too-short buffer rather than read past its end"
        );
    }

    #[test]
    fn detect_still_works_on_a_correctly_sized_frame() {
        let model_path = crate::default_models_dir().join("det_500m.onnx");
        // See the comment in the test above: check existence first, since
        // `load` can deadlock rather than error when the onnxruntime dylib
        // itself is missing.
        if !model_path.exists() {
            eprintln!(
                "Skipping: SCRFD model not available at {}",
                model_path.display()
            );
            return;
        }
        let Ok(detector) = ScrfdDetector::load(&model_path) else {
            eprintln!(
                "Skipping: SCRFD model not available at {}",
                model_path.display()
            );
            return;
        };
        let frame = vec![128u8; 640 * 480 * 3];
        let result = detector.detect(&frame, 640, 480, 3);
        assert!(
            result.is_ok(),
            "a correctly sized frame must not be rejected"
        );
    }
}
