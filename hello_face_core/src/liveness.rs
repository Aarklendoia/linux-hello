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
    // Every read below indexes gray_frame assuming it holds at least w*h
    // bytes, with no bounds check of its own (normalize_roi and the
    // raw_range loop just below both do `gray_frame[(y * w + x) as usize]`
    // unconditionally). w/h come from the caller (ultimately the
    // V4L2-negotiated capture format) while gray_frame is a separately
    // produced buffer that can legitimately be shorter (a truncated
    // capture, or a driver/format mismatch — capture_gray_stream_v4l2
    // passes the raw mmap buffer through with no stride/size adjustment of
    // its own). Same "no decision" sentinel already used a few lines below
    // for other degenerate inputs (tiny ROI, low dynamic range), rather
    // than introducing a Result return here.
    if (gray_frame.len() as u64) < (w as u64) * (h as u64) {
        return 0.5;
    }

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
    let (normalized, roi_min, roi_max) = normalize_roi(gray_frame, w, x1, y1, x2, y2);
    let norm_frame: &[u8] = &normalized;
    let norm_w = x2 - x1;
    let norm_h = y2 - y1;

    // If the dynamic range is too low, the IR image is unusable
    // (camera out of frame, shutter closed, no signal at all).
    // Return 0.5 = no decision rather than penalizing.
    let raw_range = roi_max.saturating_sub(roi_min);
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

/// Fallback liveness score for cameras with no IR channel, based on texture/
/// gradient statistics of the RGB face ROI alone.
///
/// # Calibration and honest limits
///
/// This does **not** reuse the IR heuristic's assumption that "more texture
/// = more likely real" — real-camera testing showed the opposite for the
/// attack this targets. Measured once, on one real Windows-Hello-class
/// RGB+IR webcam, one subject, one phone (`hello_daemon/examples/
/// liveness_calibration.rs`, since removed after use):
///
/// | sample                        | Laplacian var | gradient  |
/// |--------------------------------|---------------|-----------|
/// | live face (5 frames)          | 250-275       | 0.19      |
/// | phone-screen replay (10 frames, 2 sessions) | 795-1080 | 0.35-0.37 |
///
/// A phone/monitor replaying a photo reads as *more* textured than skin —
/// screen pixel-grid moiré and a sharper source image outweigh the
/// smoothing a re-capture would otherwise cause. So this scores high
/// texture/gradient as the suspicious direction, the mirror image of
/// [`ir_liveness_score`].
///
/// This is a coarse triage for blatant screen-replay artifacts, not a
/// validated anti-spoofing guarantee: it was calibrated against a single
/// subject and a single attack sample, so the accept band is set with
/// deliberate headroom above the one live sample observed (favoring false
/// accepts over rejecting a legitimate user under different lighting/skin/
/// camera) rather than splitting the gap tightly. It has not been tested
/// against a printed photo (paper texture/halftone could plausibly score
/// either direction) or against any spoof technique that doesn't elevate
/// texture. Deliberately excludes color/saturation, even though the
/// real-hardware sample showed a difference there too — that signal
/// correlates with skin tone and would risk a biased false-reject rate
/// across users, for a benefit that's already covered by the texture/
/// gradient bands here.
///
/// See docs/PAM_MODULE.md for how this is wired into the verification gate.
pub fn rgb_liveness_score(rgb_frame: &[u8], w: u32, h: u32, face: &FaceRegion) -> f32 {
    let Some(gray) = rgb_to_gray(rgb_frame, w, h) else {
        return 0.5; // malformed/truncated buffer -> no decision
    };

    let (fx, fy, fw, fh) = face.bounding_box;
    let x1 = fx.min(w.saturating_sub(1));
    let y1 = fy.min(h.saturating_sub(1));
    let x2 = (fx + fw).min(w);
    let y2 = (fy + fh).min(h);
    if x2 <= x1 + 4 || y2 <= y1 + 4 {
        return 0.5; // ROI too small -> no decision
    }

    let lap_var = laplacian_variance(&gray, w, h, x1, y1, x2, y2);
    let grad = gradient_mean(&gray, w, h, x1, y1, x2, y2);

    tracing::debug!("Liveness RGB: lap_var={:.1}, gradient={:.4}", lap_var, grad);

    // Bands sit between the observed live sample and the observed spoof
    // samples above, biased toward the live side (see doc comment).
    let tex_ok = 1.0 - sigmoid_score(lap_var, 650.0, 800.0);
    let grad_ok = 1.0 - sigmoid_score(grad, 0.25, 0.32);

    (0.6 * tex_ok + 0.4 * grad_ok).clamp(0.0, 1.0)
}

/// Converts an interleaved RGB888 buffer to 8-bit grayscale (ITU-R BT.601
/// luma weights), for [`rgb_liveness_score`]. Returns `None` if `rgb` is
/// shorter than `w * h * 3` — same defensive posture as the IR path
/// (`ir_liveness_score`) against a truncated/mismatched capture.
fn rgb_to_gray(rgb: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let expected = (w as u64) * (h as u64) * 3;
    if (rgb.len() as u64) < expected {
        return None;
    }
    let mut gray = Vec::with_capacity((w * h) as usize);
    for chunk in rgb[..expected as usize].chunks_exact(3) {
        let r = chunk[0] as u32;
        let g = chunk[1] as u32;
        let b = chunk[2] as u32;
        gray.push(((r * 77 + g * 150 + b * 29) >> 8) as u8);
    }
    Some(gray)
}

/// Extracts the ROI and normalizes the pixels to [0, 255] (contrast
/// stretching). Also returns the pre-normalization (min, max) — the caller
/// needs those too (as `raw_range`), and this is the only place that already
/// scans every ROI pixel, so it hands them back instead of making the
/// caller re-scan the same pixels a second time just to recompute them.
fn normalize_roi(gray: &[u8], w: u32, x1: u32, y1: u32, x2: u32, y2: u32) -> (Vec<u8>, u8, u8) {
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
        return (roi, mn, mx); // uniform image, normalization not possible
    }

    for v in roi.iter_mut() {
        *v = ((*v as u32 - mn as u32) * 255 / range as u32) as u8;
    }
    (roi, mn, mx)
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

    #[test]
    fn test_liveness_rejects_a_truncated_gray_frame_instead_of_reading_out_of_bounds() {
        // Declares 64x64 but only actually supplies 10 bytes — exactly the
        // mismatch a truncated IR capture or a driver/format mismatch would
        // produce (capture_gray_stream_v4l2 passes the raw mmap buffer
        // through unadjusted). Must return the "no decision" sentinel
        // rather than panicking.
        let w = 64u32;
        let h = 64u32;
        let truncated = vec![120u8; 10];
        let face = FaceRegion {
            bounding_box: (8, 8, 48, 48),
            confidence: 0.9,
            landmarks: vec![],
        };
        let score = ir_liveness_score(&truncated, w, h, &face);
        assert_eq!(
            score, 0.5,
            "a too-short buffer must yield the no-decision sentinel"
        );
    }

    #[test]
    fn test_rgb_liveness_smooth_image_scores_high_like_a_real_face() {
        // Low-frequency RGB content, standing in for the real "live face"
        // calibration sample (lap_var ~260, gradient ~0.19) — see the doc
        // comment on rgb_liveness_score for the real numbers this is based
        // on. Should land near the "looks real" end.
        let w = 64u32;
        let h = 64u32;
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let v = (120 + (x as i32 - 32) / 4 + (y as i32 - 32) / 8) as u8;
                rgb.push(v);
                rgb.push(v);
                rgb.push(v);
            }
        }
        let face = FaceRegion {
            bounding_box: (8, 8, 48, 48),
            confidence: 0.9,
            landmarks: vec![],
        };
        let score = rgb_liveness_score(&rgb, w, h, &face);
        assert!(
            score > 0.8,
            "smooth/low-frequency content should score high: {}",
            score
        );
    }

    #[test]
    fn test_rgb_liveness_high_frequency_image_scores_low_like_a_screen_replay() {
        // Pseudo-random high-frequency content (same generator as
        // test_liveness_noisy_image above), standing in for the moiré/
        // screen-grid aliasing measured on a real phone-screen replay
        // (lap_var ~800-1080, gradient ~0.35-0.37 in the real sample). A
        // perfect axis-aligned checkerboard was tried first and rejected:
        // its Sobel gx/gy cancel out by symmetry at every pixel, which is
        // an artifact of that specific synthetic pattern, not of real
        // moiré — pseudo-random noise doesn't have that degenerate
        // cancellation.
        let w = 64u32;
        let h = 64u32;
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let v = ((i * 37 + i / 8) % 200 + 50) as u8;
                rgb.push(v);
                rgb.push(v);
                rgb.push(v);
            }
        }
        let face = FaceRegion {
            bounding_box: (8, 8, 48, 48),
            confidence: 0.9,
            landmarks: vec![],
        };
        let score = rgb_liveness_score(&rgb, w, h, &face);
        assert!(
            score < 0.2,
            "high-frequency/pseudo-random content should score low: {}",
            score
        );
    }

    #[test]
    fn test_rgb_liveness_rejects_a_truncated_frame_instead_of_reading_out_of_bounds() {
        // Same defensive posture as ir_liveness_score: a buffer shorter
        // than w*h*3 must yield the no-decision sentinel, not panic.
        let w = 64u32;
        let h = 64u32;
        let truncated = vec![120u8; 10];
        let face = FaceRegion {
            bounding_box: (8, 8, 48, 48),
            confidence: 0.9,
            landmarks: vec![],
        };
        let score = rgb_liveness_score(&truncated, w, h, &face);
        assert_eq!(
            score, 0.5,
            "a too-short buffer must yield the no-decision sentinel"
        );
    }
}
