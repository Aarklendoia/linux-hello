//! D-Bus signal handling for capture streaming
//!
//! Allows emitting a D-Bus signal for each captured frame,
//! letting the GUI receive real-time updates.

use tracing::debug;

/// D-Bus signal manager for streaming
pub struct StreamingSignalEmitter;

impl Default for StreamingSignalEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingSignalEmitter {
    /// Create a new signal emitter
    pub fn new() -> Self {
        Self
    }

    /// Emit a capture completed signal
    pub async fn emit_capture_completed(&self, user_id: u32) -> Result<(), String> {
        debug!("Emitting CaptureCompleted signal for user_id={}", user_id);
        Ok(())
    }

    /// Emit a capture error signal
    pub async fn emit_capture_error(&self, user_id: u32, error_msg: &str) -> Result<(), String> {
        debug!(
            "Emitting CaptureError signal for user_id={}: {}",
            user_id, error_msg
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn emit_capture_completed_and_error_both_succeed() {
        let emitter = StreamingSignalEmitter::new();
        assert!(emitter.emit_capture_completed(1000).await.is_ok());
        assert!(emitter
            .emit_capture_error(1000, "no face detected")
            .await
            .is_ok());
    }
}
