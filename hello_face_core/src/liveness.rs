//! Liveness detection via IR camera analysis
//!
//! ML-model-free algorithm: Laplacian variance in the face ROI.
//!
//! Physical principle:
//! - Real face in front of the Brio's IR camera: natural thermal gradient,
//!   rich skin texture, uniform IR distribution with relief.
//! - Printed photo: dithering artifacts, sharp edges, low variance,
//!   no thermal gradient.
//!
//! Returned score: f32 in \[0, 1\]
//!   - `> 0.6` -> likely a real face
//!   - `< 0.3` -> likely a photo / presentation attack

use crate::FaceRegion;

/// Liveness score based on the IR camera
///
/// # Arguments
/// * `gray_frame` — 8-bit GREY data of the IR frame
/// * `w`, `h` — frame dimensions
/// * `face` — detected face region (from SCRFD on the RGB frame)
///
/// # Returns
/// Liveness score between 0.0 and 1.0
pub fn ir_liveness_score(gray_frame: &[u8], w: u32, h: u32, face: &FaceRegion) -> f32 {
    let (fx, fy, fw, fh) = face.bounding_box;

    // Clamp the ROI to the image bounds
    let x1 = fx.min(w.saturating_sub(1));
    let y1 = fy.min(h.saturating_sub(1));
    let x2 = (fx + fw).min(w);
    let y2 = (fy + fh).min(h);

    if x2 <= x1 + 4 || y2 <= y1 + 4 {
        return 0.5; // ROI too small -> no decision
    }

    // Normalize the ROI pixels to [0, 255].
    // Makes the algorithm independent of the IR camera's exposure level
    // (e.g. camera without active illuminator -> raw values ~30, Brio -> ~120).
    let normalized = normalize_roi(gray_frame, w, x1, y1, x2, y2);
    let norm_frame: &[u8] = &normalized;
    let norm_w = x2 - x1;
    let norm_h = y2 - y1;

    // If the dynamic range is too low, the IR image is unusable
    // (camera out of frame, shutter closed, no signal at all).
    // Return 0.5 = no decision rather than penalizing.
    let raw_range = {
        let mut mn = 255u8;
        let mut mx = 0u8;
        for y in y1..y2 {
            for x in x1..x2 {
                let v = gray_frame[(y * w + x) as usize];
                mn = mn.min(v);
                mx = mx.max(v);
            }
        }
        mx.saturating_sub(mn)
    };
    if raw_range < 5 {
        tracing::debug!(
            "Liveness IR: dynamic range too low ({}), no decision",
            raw_range
        );
        return 0.5;
    }

    // Work on the normalized pixels (extracted ROI, local coordinates)
    let norm_x1 = 0u32;
    let norm_y1 = 0u32;
    let norm_x2 = norm_w;
    let norm_y2 = norm_h;

    // 1. Laplacian variance — measures texture richness
    let laplacian_var = laplacian_variance(
        norm_frame, norm_w, norm_h, norm_x1, norm_y1, norm_x2, norm_y2,
    );

    // 2. Normalized intensity stats (mean ~= 128 after normalization)
    let (mean, intensity_var) =
        intensity_stats(norm_frame, norm_w, norm_x1, norm_y1, norm_x2, norm_y2);

    // 3. Mean gradient
    let gradient_score = gradient_mean(
        norm_frame, norm_w, norm_h, norm_x1, norm_y1, norm_x2, norm_y2,
    );

    tracing::debug!(
        "Liveness IR: raw_range={}, laplacian_var={:.1}, intensity_var={:.1}, mean={:.1}, gradient={:.3}",
        raw_range, laplacian_var, intensity_var, mean, gradient_score
    );

    // Thresholds calibrated on normalized pixels [0-255]:
    //   Laplacian variance: < 50 -> smooth (photo); > 500 -> textured (real face)
    let lap_score = sigmoid_score(laplacian_var, 50.0, 500.0);

    //   Normalized mean intensity: expected ~128. Too uniform = artifact.
    let thermal_score = gaussian_score(mean, 128.0, 60.0);

    //   Normalized intensity variance: moderate (500-3000) = natural distribution
    let var_score = gaussian_score(intensity_var, 1500.0, 1000.0);

    //   Gradient: > 0.04 = textures present
    let grad_score = sigmoid_score(gradient_score * 255.0, 10.0, 60.0);

    let score = 0.40 * lap_score + 0.20 * thermal_score + 0.15 * var_score + 0.25 * grad_score;
    score.clamp(0.0, 1.0)
}

/// Extracts the ROI and normalizes the pixels to [0, 255] (contrast stretching).
fn normalize_roi(gray: &[u8], w: u32, x1: u32, y1: u32, x2: u32, y2: u32) -> Vec<u8> {
    let mut roi: Vec<u8> = Vec::with_capacity(((x2 - x1) * (y2 - y1)) as usize);
    let mut mn = 255u8;
    let mut mx = 0u8;

    for y in y1..y2 {
        for x in x1..x2 {
            let v = gray[(y * w + x) as usize];
            roi.push(v);
            mn = mn.min(v);
            mx = mx.max(v);
        }
    }

    let range = mx.saturating_sub(mn);
    if range == 0 {
        return roi; // uniform image, normalization not possible
    }

    for v in roi.iter_mut() {
        *v = ((*v as u32 - mn as u32) * 255 / range as u32) as u8;
    }
    roi
}

/// Computes the 3x3 Laplacian filter variance over the ROI
fn laplacian_variance(gray: &[u8], w: u32, h: u32, x1: u32, y1: u32, x2: u32, y2: u32) -> f32 {
    let mut vals: Vec<f32> = Vec::new();

    for y in (y1 + 1)..(y2.min(h) - 1) {
        for x in (x1 + 1)..(x2.min(w) - 1) {
            let p = |dy: i32, dx: i32| -> f32 {
                let ny = (y as i32 + dy) as u32;
                let nx = (x as i32 + dx) as u32;
                gray[(ny * w + nx) as usize] as f32
            };

            // Laplacian kernel: 0 -1 0 / -1 4 -1 / 0 -1 0
            let lap = 4.0 * p(0, 0) - p(-1, 0) - p(1, 0) - p(0, -1) - p(0, 1);
            vals.push(lap);
        }
    }

    if vals.is_empty() {
        return 0.0;
    }

    let mean = vals.iter().sum::<f32>() / vals.len() as f32;
    vals.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / vals.len() as f32
}

/// Computes the mean and variance of intensity within the ROI
fn intensity_stats(gray: &[u8], w: u32, x1: u32, y1: u32, x2: u32, y2: u32) -> (f32, f32) {
    let mut sum = 0.0f32;
    let mut count = 0usize;

    for y in y1..y2 {
        for x in x1..x2 {
            sum += gray[(y * w + x) as usize] as f32;
            count += 1;
        }
    }

    if count == 0 {
        return (0.0, 0.0);
    }

    let mean = sum / count as f32;
    let var = {
        let mut sq = 0.0f32;
        for y in y1..y2 {
            for x in x1..x2 {
                let d = gray[(y * w + x) as usize] as f32 - mean;
                sq += d * d;
            }
        }
        sq / count as f32
    };

    (mean, var)
}

/// Mean gradient (Sobel magnitude) within the ROI
fn gradient_mean(gray: &[u8], w: u32, h: u32, x1: u32, y1: u32, x2: u32, y2: u32) -> f32 {
    let mut sum = 0.0f32;
    let mut count = 0usize;

    for y in (y1 + 1)..(y2.min(h) - 1) {
        for x in (x1 + 1)..(x2.min(w) - 1) {
            let p = |dy: i32, dx: i32| -> f32 {
                let ny = (y as i32 + dy) as u32;
                let nx = (x as i32 + dx) as u32;
                gray[(ny * w + nx) as usize] as f32
            };

            let gx = p(-1, 1) + 2.0 * p(0, 1) + p(1, 1) - p(-1, -1) - 2.0 * p(0, -1) - p(1, -1);
            let gy = p(1, -1) + 2.0 * p(1, 0) + p(1, 1) - p(-1, -1) - 2.0 * p(-1, 0) - p(-1, 1);
            sum += (gx * gx + gy * gy).sqrt() / 255.0;
            count += 1;
        }
    }

    if count == 0 {
        0.0
    } else {
        sum / count as f32
    }
}

/// Maps a value to a [0,1] score via a linear sigmoid between low and high
#[inline]
fn sigmoid_score(val: f32, low: f32, high: f32) -> f32 {
    ((val - low) / (high - low)).clamp(0.0, 1.0)
}

/// Gaussian score centered on `center` with standard deviation `sigma`
#[inline]
fn gaussian_score(val: f32, center: f32, sigma: f32) -> f32 {
    let d = (val - center) / sigma;
    (-0.5 * d * d).exp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FaceRegion;

    #[test]
    fn test_liveness_flat_image() {
        // Flat IR image (plain paper) -> low score
        let w = 64u32;
        let h = 64u32;
        let gray = vec![120u8; (w * h) as usize]; // uniform intensity
        let face = FaceRegion {
            bounding_box: (8, 8, 48, 48),
            confidence: 0.9,
            landmarks: vec![],
        };
        let score = ir_liveness_score(&gray, w, h, &face);
        assert!(
            score < 0.55,
            "Flat image should give a low score: {}",
            score
        );
    }

    #[test]
    fn test_liveness_noisy_image() {
        // IR image with high variance (simulated real face) -> high score
        let w = 64u32;
        let h = 64u32;
        let gray: Vec<u8> = (0..(w * h) as usize)
            .map(|i| ((i * 37 + i / 8) % 200 + 50) as u8)
            .collect();
        let face = FaceRegion {
            bounding_box: (8, 8, 48, 48),
            confidence: 0.9,
            landmarks: vec![],
        };
        let score = ir_liveness_score(&gray, w, h, &face);
        assert!(
            score > 0.4,
            "Textured image should give a correct score: {}",
            score
        );
    }
}
