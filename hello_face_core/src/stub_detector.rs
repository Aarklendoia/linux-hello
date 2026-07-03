//! Fast stub detector for live preview
//!
//! Just detects the bounding box without full extraction
//! Used for live streaming (low latency)

use super::{FaceDetector, FaceError, FaceRegion};

/// Stub detector - simulates a fast detection
/// To be replaced by real YOLO or RetinaFace
pub struct StubDetector {
    name: String,
    version: String,
}

impl StubDetector {
    pub fn new() -> Self {
        Self {
            name: "stub-detector".to_string(),
            version: "0.1.0".to_string(),
        }
    }
}

impl Default for StubDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl FaceDetector for StubDetector {
    fn detect(
        &self,
        frame_data: &[u8],
        width: u32,
        height: u32,
        channels: u32,
    ) -> Result<Vec<FaceRegion>, FaceError> {
        // Stub: simulate a simple detection
        // In production: use YOLO, RetinaFace, etc.

        // Basic validation
        if frame_data.is_empty() || width == 0 || height == 0 || channels == 0 {
            return Ok(Vec::new());
        }

        let expected_size = (width * height * channels) as usize;
        if frame_data.len() < expected_size {
            return Err(FaceError::InvalidFrame(format!(
                "Invalid frame size: {} < {}",
                frame_data.len(),
                expected_size
            )));
        }

        // Stub: detect a face in the center (for testing)
        // Simulates: detection of a square face in the center 200x200px
        let face_width = (width as f32 * 0.3) as u32;
        let face_height = (height as f32 * 0.4) as u32;
        let face_x = (width - face_width) / 2;
        let face_y = (height - face_height) / 2;

        // Detection based on simple contrast (stub)
        let face_detected = self.detect_simple_contrast(
            frame_data,
            width,
            height,
            channels,
            face_x,
            face_y,
            face_width,
            face_height,
        );

        if face_detected {
            Ok(vec![FaceRegion {
                bounding_box: (face_x, face_y, face_width, face_height),
                confidence: 0.85, // stub confidence
                landmarks: vec![],
            }])
        } else {
            Ok(Vec::new())
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn model_version(&self) -> &str {
        &self.version
    }
}

impl StubDetector {
    /// Detection based on simple contrast
    #[allow(clippy::too_many_arguments)]
    fn detect_simple_contrast(
        &self,
        frame_data: &[u8],
        _width: u32,
        _height: u32,
        channels: u32,
        face_x: u32,
        face_y: u32,
        face_width: u32,
        face_height: u32,
    ) -> bool {
        // Compute an average contrast in the assumed face region
        let mut total = 0u32;
        let mut count = 0u32;

        for y in face_y..face_y + face_height {
            for x in face_x..face_x + face_width {
                let idx = ((y * _width + x) * channels) as usize;

                // Use the red channel (or average)
                if idx < frame_data.len() {
                    total += frame_data[idx] as u32;
                    count += 1;
                }
            }
        }

        if count == 0 {
            return false;
        }

        let avg = total / count;
        // Stub: determine if there is "enough contrast"
        // Range [50, 200] is considered a face
        avg > 50 && avg < 200
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_detector_creation() {
        let detector = StubDetector::new();
        assert_eq!(detector.name(), "stub-detector");
    }

    #[test]
    fn test_stub_detector_invalid_frame() {
        let detector = StubDetector::new();
        let result = detector.detect(&[], 0, 0, 3);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_stub_detector_empty_frame() {
        let detector = StubDetector::new();
        let frame = vec![0u8; 640 * 480 * 3];
        let result = detector.detect(&frame, 640, 480, 3).unwrap();
        // With all black pixels, no detection
        assert!(result.is_empty());
    }
}
