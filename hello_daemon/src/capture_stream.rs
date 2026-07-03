//! Types and structures for live capture streaming
//!
//! Provides the events and structures to display a real-time
//! preview with face detection

use serde::{Deserialize, Serialize};

/// Event for a frame captured during enrollment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureFrameEvent {
    /// Frame number (0-indexed)
    pub frame_number: u32,

    /// Total number of frames to capture
    pub total_frames: u32,

    /// Raw RGB data (640x480x3)
    /// Note: for D-Bus transmission, may be compressed to JPEG
    pub frame_data: Vec<u8>,

    /// Image width
    pub width: u32,

    /// Image height
    pub height: u32,

    /// Was a face detected?
    pub face_detected: bool,

    /// Bounding box of the detected face (x, y, width, height)
    /// None if no face
    pub face_box: Option<FaceBox>,

    /// Quality score of this frame (0.0-1.0)
    pub quality_score: f32,

    /// Capture timestamp (ms since start)
    pub timestamp_ms: u64,
}

impl CaptureFrameEvent {
    /// Create a new frame event
    pub fn new(frame_number: u32, total_frames: u32, width: u32, height: u32) -> Self {
        Self {
            frame_number,
            total_frames,
            frame_data: Vec::new(),
            width,
            height,
            face_detected: false,
            face_box: None,
            quality_score: 0.0,
            timestamp_ms: 0,
        }
    }

    /// Progress in percent (0-100)
    pub fn progress_percent(&self) -> u32 {
        if self.total_frames == 0 {
            return 0;
        }
        ((self.frame_number + 1) * 100) / self.total_frames
    }

    /// Is this the last frame?
    pub fn is_last_frame(&self) -> bool {
        self.frame_number + 1 >= self.total_frames
    }
}

/// Bounding box of a detected face
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct FaceBox {
    /// X position of the top-left corner
    pub x: u32,

    /// Y position of the top-left corner
    pub y: u32,

    /// Rectangle width
    pub width: u32,

    /// Rectangle height
    pub height: u32,

    /// Detection confidence (0.0-1.0)
    pub confidence: f32,
}

impl FaceBox {
    /// Create a new bounding box
    pub fn new(x: u32, y: u32, width: u32, height: u32, confidence: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            confidence,
        }
    }

    /// Center the box within an image
    pub fn center(&self) -> (u32, u32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    /// Check whether a point is inside the box
    pub fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// State of a capture session
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    /// Session not initialized
    Idle,

    /// Waiting for camera placement
    Waiting,

    /// Capture in progress
    Capturing,

    /// Capture completed successfully
    Completed,

    /// Error during capture
    Failed,

    /// Capture cancelled by the user
    Cancelled,
}

impl std::fmt::Display for CaptureState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureState::Idle => write!(f, "Idle"),
            CaptureState::Waiting => write!(f, "Waiting"),
            CaptureState::Capturing => write!(f, "Capturing"),
            CaptureState::Completed => write!(f, "Completed"),
            CaptureState::Failed => write!(f, "Error"),
            CaptureState::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Configuration for a capture session
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Number of frames to capture
    pub num_frames: u32,

    /// Total timeout in milliseconds (0 = infinite)
    pub timeout_ms: u64,

    /// Minimum confidence threshold for detection (0.0-1.0)
    pub detection_confidence_threshold: f32,

    /// Minimum quality threshold (0.0-1.0)
    pub quality_threshold: f32,

    /// Accept frames without a face?
    pub accept_no_face: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            num_frames: 30,
            timeout_ms: 120000, // 2 minutes
            detection_confidence_threshold: 0.6,
            quality_threshold: 0.5,
            accept_no_face: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_frame_event_progress() {
        let mut event = CaptureFrameEvent::new(5, 30, 640, 480);
        assert_eq!(event.progress_percent(), 20); // (5+1)*100/30 = 20

        event.frame_number = 29;
        assert_eq!(event.progress_percent(), 100);
    }

    #[test]
    fn test_face_box_contains() {
        let face = FaceBox::new(100, 100, 50, 50, 0.9);
        assert!(face.contains(125, 125));
        assert!(!face.contains(50, 50));
        assert!(!face.contains(150, 150));
    }

    #[test]
    fn test_face_box_center() {
        let face = FaceBox::new(100, 100, 50, 50, 0.9);
        assert_eq!(face.center(), (125, 125));
    }
}
