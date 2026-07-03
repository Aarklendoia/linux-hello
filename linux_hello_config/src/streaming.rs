//! Capture streaming management from the daemon
//!
//! Module for receiving and processing frames in real time

use serde::{Deserialize, Serialize};

/// Frame event received from the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureFrame {
    /// Frame number (0-indexed)
    pub frame_number: u32,

    /// Total number of expected frames
    pub total_frames: u32,

    /// Raw RGB data (640×480×3 or other)
    pub frame_data: Vec<u8>,

    /// Image width
    pub width: u32,

    /// Image height
    pub height: u32,

    /// Was a face detected?
    pub face_detected: bool,

    /// Bounding box if a face was detected
    pub face_box: Option<FaceBox>,

    /// Quality score (0.0-1.0)
    pub quality_score: f32,

    /// Capture timestamp (ms)
    pub timestamp_ms: u64,
}

/// Bounding box of a detected face
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceBox {
    /// X position in pixels
    pub x: u32,
    /// Y position in pixels
    pub y: u32,
    /// Box width in pixels
    pub width: u32,
    /// Box height in pixels
    pub height: u32,
    /// Confidence score (0.0-1.0)
    pub confidence: f32,
}

impl FaceBox {
    /// Check whether a point is inside the bounding box
    pub fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Return the center of the bounding box
    pub fn center(&self) -> (u32, u32) {
        (
            self.x + self.width / 2,
            self.y + self.height / 2,
        )
    }

    /// Calculate the completion percentage based on the frame
    pub fn completion_percent(&self, frame_num: u32, total_frames: u32) -> f32 {
        if total_frames == 0 {
            0.0
        } else {
            (frame_num as f32 / total_frames as f32) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_face_box_contains() {
        let face_box = FaceBox {
            x: 100,
            y: 100,
            width: 200,
            height: 200,
            confidence: 0.9,
        };

        assert!(face_box.contains(150, 150)); // Center
        assert!(face_box.contains(100, 100)); // Top-left corner
        assert!(!face_box.contains(99, 100)); // Outside left
        assert!(!face_box.contains(300, 150)); // Outside right
    }

    #[test]
    fn test_face_box_center() {
        let face_box = FaceBox {
            x: 100,
            y: 100,
            width: 200,
            height: 200,
            confidence: 0.9,
        };

        let (cx, cy) = face_box.center();
        assert_eq!(cx, 200);
        assert_eq!(cy, 200);
    }

    #[test]
    fn test_completion_percent() {
        let face_box = FaceBox {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            confidence: 0.9,
        };

        let percent = face_box.completion_percent(10, 30);
        assert!((percent - 33.33).abs() < 0.1);
    }
}
