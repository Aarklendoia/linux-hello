//! D-Bus surface for FaceAuthDaemon
//!
//! Wrapper that exposes the daemon's operations via D-Bus

use crate::dbus_interface::{DeleteFaceRequest, RegisterFaceRequest, VerifyRequest};
use crate::dbus_signals::StreamingSignalEmitter;
use crate::FaceAuthDaemon;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use zbus::{interface, Connection};

/// D-Bus wrapper around the daemon
pub struct FaceAuthInterface {
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    signal_emitter: Option<Arc<StreamingSignalEmitter>>,
    version: String,
    storage_path: String,
}

impl FaceAuthInterface {
    /// Create a new interface without a signal emitter (backward compatible)
    pub fn new(daemon: FaceAuthDaemon) -> Self {
        let storage_path = daemon.config().storage_path.to_string_lossy().into_owned();
        Self {
            daemon: Arc::new(RwLock::new(daemon)),
            signal_emitter: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            storage_path,
        }
    }

    /// Create a new interface with a D-Bus signal emitter
    pub fn new_with_connection(daemon: FaceAuthDaemon, connection: Connection) -> Self {
        let storage_path = daemon.config().storage_path.to_string_lossy().into_owned();
        let signal_emitter = Arc::new(StreamingSignalEmitter::new(Arc::new(connection)));
        Self {
            daemon: Arc::new(RwLock::new(daemon)),
            signal_emitter: Some(signal_emitter),
            version: env!("CARGO_PKG_VERSION").to_string(),
            storage_path,
        }
    }

    /// Create from a shared Arc (allows sharing the daemon with the PAM helper)
    pub fn from_arc(
        daemon: Arc<RwLock<FaceAuthDaemon>>,
        storage_path: String,
        connection: Connection,
    ) -> Self {
        let signal_emitter = Arc::new(StreamingSignalEmitter::new(Arc::new(connection)));
        Self {
            daemon,
            signal_emitter: Some(signal_emitter),
            version: env!("CARGO_PKG_VERSION").to_string(),
            storage_path,
        }
    }
}

#[interface(name = "com.linuxhello.FaceAuth")]
impl FaceAuthInterface {
    /// Register a new face for a user
    ///
    /// # Arguments
    /// * `request_json` - JSON string of RegisterFaceRequest
    ///
    /// # Returns
    /// JSON string of RegisterFaceResponse or error
    pub async fn register_face(&self, request_json: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: register_face");

        let request: RegisterFaceRequest = match serde_json::from_str(request_json) {
            Ok(r) => r,
            Err(e) => {
                error!("JSON parse error: {}", e);
                return Err(zbus::fdo::Error::Failed(format!("JSON parse error: {}", e)));
            }
        };

        let daemon = self.daemon.write().await;
        let response = daemon.register_face(request).await;

        match response {
            Ok(response_json) => {
                info!("register_face succeeded");
                Ok(response_json)
            }
            Err(e) => {
                error!("register_face failed: {}", e);
                Err(zbus::fdo::Error::Failed(e.to_string()))
            }
        }
    }

    /// Delete one or all faces
    ///
    /// # Arguments
    /// * `request_json` - JSON string of DeleteFaceRequest
    pub async fn delete_face(&self, request_json: &str) -> zbus::fdo::Result<()> {
        debug!("D-Bus call: delete_face");

        let request: DeleteFaceRequest = match serde_json::from_str(request_json) {
            Ok(r) => r,
            Err(e) => {
                error!("JSON parse error: {}", e);
                return Err(zbus::fdo::Error::Failed(format!("JSON parse error: {}", e)));
            }
        };

        let daemon = self.daemon.write().await;
        let response = daemon.delete_face(request).await;

        match response {
            Ok(_) => {
                info!("delete_face succeeded");
                Ok(())
            }
            Err(e) => {
                error!("delete_face failed: {}", e);
                Err(zbus::fdo::Error::Failed(e.to_string()))
            }
        }
    }

    /// Verify a user's identity
    ///
    /// # Arguments
    /// * `request_json` - JSON string of VerifyRequest
    ///
    /// # Returns
    /// JSON string of VerifyResult
    pub async fn verify(&self, request_json: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: verify");

        let request: VerifyRequest = match serde_json::from_str(request_json) {
            Ok(r) => r,
            Err(e) => {
                error!("JSON parse error: {}", e);
                return Err(zbus::fdo::Error::Failed(format!("JSON parse error: {}", e)));
            }
        };

        let daemon = self.daemon.write().await;
        let result = daemon.verify(request).await;

        match result {
            Ok(result) => {
                let result_json = match serde_json::to_string(&result) {
                    Ok(j) => j,
                    Err(e) => {
                        error!("JSON serialize error: {}", e);
                        return Err(zbus::fdo::Error::Failed(e.to_string()));
                    }
                };
                info!("verify succeeded");
                Ok(result_json)
            }
            Err(e) => {
                error!("verify failed: {}", e);
                Err(zbus::fdo::Error::Failed(e.to_string()))
            }
        }
    }

    /// List the faces registered for a user
    ///
    /// # Arguments
    /// * `user_id` - User UID
    ///
    /// # Returns
    /// JSON array of FaceRecord
    pub async fn list_faces(&self, user_id: u32) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: list_faces for user_id={}", user_id);

        let daemon = self.daemon.write().await;
        let result = daemon.list_faces(user_id).await;

        match result {
            Ok(faces_json) => {
                info!("list_faces succeeded");
                Ok(faces_json)
            }
            Err(e) => {
                error!("list_faces failed: {}", e);
                Err(zbus::fdo::Error::Failed(e.to_string()))
            }
        }
    }

    /// Connection test
    pub async fn ping(&self) -> zbus::fdo::Result<String> {
        Ok("pong".to_string())
    }

    /// Start a streaming capture session with signal emission
    ///
    /// Emits `CaptureProgress` D-Bus signals for each captured frame.
    /// The GUI subscribes to these signals to display the live preview.
    ///
    /// # Arguments
    /// * `user_id` - UID of the user enrolling
    /// * `num_frames` - Number of frames to capture (30 by default)
    /// * `timeout_ms` - Timeout in milliseconds (120000 by default = 2 minutes)
    ///
    /// # Returns
    /// "OK" if the capture started successfully, or an error
    ///
    /// # D-Bus Signal Emitted
    /// `CaptureProgress(event_json: &str)` - Emitted for each frame
    pub async fn start_capture_stream(
        &self,
        user_id: u32,
        num_frames: u32,
        timeout_ms: u64,
    ) -> zbus::fdo::Result<String> {
        debug!(
            "D-Bus call: start_capture_stream user_id={} num_frames={} timeout={}ms",
            user_id, num_frames, timeout_ms
        );

        info!(
            "Starting streaming capture: user_id={}, {} frames",
            user_id, num_frames
        );

        // Use the camera manager to capture in streaming mode
        let daemon = self.daemon.read().await;
        let camera_manager = daemon.camera_manager();

        // Clone the signal emitter for use in the closure
        let _signal_emitter = self.signal_emitter.clone();

        // Capture the frames with a callback that emits the signals
        let result = camera_manager
            .start_capture_stream(num_frames, timeout_ms, move |event| {
                info!(
                    "Callback: Frame {}/{} received - {} bytes",
                    event.frame_number + 1,
                    event.total_frames,
                    event.frame_data.len()
                );
                // Export the frame to JPEG for the GUI preview
                if let Err(e) = crate::preview::export_preview_frame_rgb(
                    &event.frame_data,
                    event.width,
                    event.height,
                ) {
                    error!("Preview frame export error: {}", e);
                }
            })
            .await;

        drop(daemon); // Release the lock

        match result {
            Ok(_) => {
                info!("start_capture_stream succeeded");

                // Emit the completion signal
                if let Some(emitter) = &self.signal_emitter {
                    if let Err(e) = emitter.emit_capture_completed(user_id).await {
                        error!("CaptureCompleted emission error: {}", e);
                    }
                }

                Ok("OK".to_string())
            }
            Err(e) => {
                error!("start_capture_stream failed: {}", e);

                // Emit the error signal
                if let Some(emitter) = &self.signal_emitter {
                    let error_msg = format!("{}", e);
                    if let Err(e) = emitter.emit_capture_error(user_id, &error_msg).await {
                        error!("CaptureError emission error: {}", e);
                    }
                }

                Err(zbus::fdo::Error::Failed(e.to_string()))
            }
        }
    }

    #[zbus(property)]
    pub fn version(&self) -> String {
        self.version.clone()
    }

    /// Check whether a camera is available
    #[zbus(property)]
    pub fn camera_available(&self) -> bool {
        // Use try_read so as not to block
        // On error, assume it is available
        self.daemon
            .try_read()
            .map(|daemon| daemon.is_camera_available())
            .unwrap_or(true)
    }

    /// Root or user mode
    #[zbus(property)]
    pub fn root_mode(&self) -> bool {
        self.daemon
            .try_read()
            .map(|daemon| daemon.config().root_mode)
            .unwrap_or(false)
    }

    /// Storage path
    #[zbus(property)]
    pub fn storage_path(&self) -> String {
        self.storage_path.clone()
    }
}
