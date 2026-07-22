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

    /// Quality score of this frame (0.0-1.0)
    pub quality_score: f32,

    /// Capture timestamp (ms since start)
    pub timestamp_ms: u64,
}
