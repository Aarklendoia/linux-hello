//! Config module - configuration management
//!
//! Responsible for:
//! - Enrollment settings (frame count, timeouts)
//! - Detection settings (thresholds, models)
//! - Configuration storage

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use dirs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    /// Number of frames to capture for enrollment
    pub enrollment_frame_count: u32,

    /// Maximum timeout for enrollment (seconds)
    pub enrollment_timeout_secs: u64,

    /// Minimum confidence threshold for detection
    pub detection_confidence_threshold: f32,

    /// Minimum quality threshold
    pub quality_threshold: f32,

    /// Camera device (e.g. /dev/video0)
    pub camera_device: String,

    /// Directory for storing embeddings
    pub storage_path: PathBuf,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            enrollment_frame_count: 30,
            enrollment_timeout_secs: 120,
            detection_confidence_threshold: 0.6,
            quality_threshold: 0.5,
            camera_device: "/dev/video0".to_string(),
            storage_path: dirs::config_dir()
                .unwrap_or_default()
                .join("linux-hello"),
        }
    }
}

impl GuiConfig {
    pub fn load() -> anyhow::Result<Self> {
        // TODO: Load from config file
        Ok(Self::default())
    }

    pub fn save(&self) -> anyhow::Result<()> {
        // TODO: Save to config file
        Ok(())
    }
}
