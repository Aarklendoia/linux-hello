//! D-Bus surface for FaceAuthDaemon
//!
//! Wrapper that exposes the daemon's operations via D-Bus

use crate::dbus_interface::{DeleteFaceRequest, RegisterFaceRequest, VerifyRequest};
use crate::dbus_signals::StreamingSignalEmitter;
use crate::FaceAuthDaemon;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use zbus::interface;

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

    /// Create from a shared Arc (allows sharing the daemon with the PAM helper)
    pub fn from_arc(daemon: Arc<RwLock<FaceAuthDaemon>>, storage_path: String) -> Self {
        let signal_emitter = Arc::new(StreamingSignalEmitter::new());
        Self {
            daemon,
            signal_emitter: Some(signal_emitter),
            version: env!("CARGO_PKG_VERSION").to_string(),
            storage_path,
        }
    }
}

// All four methods below take `self.daemon.read().await`, not `.write()`:
// FaceAuthDaemon::register_face/delete_face/verify/list_faces all take
// `&self` (storage/camera/matcher are internally Arc-shared, not mutated
// through this lock), so an exclusive write lock only serialized unrelated
// requests behind whichever one got there first — most costly for verify(),
// which can hold the lock for the whole multi-second capture window,
// blocking even a concurrent read-only list_faces(). pam_helper's own
// verify() call already used a read lock; this just makes the D-Bus surface
// consistent with it.
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

        let daemon = self.daemon.read().await;
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

        let daemon = self.daemon.read().await;
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

        let daemon = self.daemon.read().await;
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

        let daemon = self.daemon.read().await;
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

    /// Whether the active camera has an infrared channel.
    ///
    /// Without one, `matcher::match_with_liveness` falls back to a weaker,
    /// RGB-only liveness heuristic instead of the well-validated IR gate
    /// (see its doc comment and `hello_face_core::liveness::rgb_liveness_score`).
    /// The GUI uses this to warn users on a camera without IR that their
    /// setup is more susceptible to a photo/video spoof than the common
    /// case — that's still true with the RGB fallback in place, just less
    /// starkly than with no check at all.
    ///
    /// # Returns
    /// JSON `{"has_ir": bool}`
    pub async fn camera_info(&self) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: camera_info");
        let daemon = self.daemon.read().await;
        let has_ir = daemon.camera_manager().has_ir();
        Ok(format!(r#"{{"has_ir":{}}}"#, has_ir))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbus_interface::{DeleteFaceRequest, RegisterFaceRequest, VerifyRequest};
    use crate::DaemonConfig;

    /// A fresh `FaceAuthInterface` backed by a tempdir, via the same `new()`
    /// (no live D-Bus `Connection` needed) used everywhere below. Every
    /// method on `FaceAuthInterface` is a plain async fn underneath the
    /// `#[interface]` macro, so it's directly callable without an actual bus.
    fn test_interface() -> (tempfile::TempDir, FaceAuthInterface) {
        let temp = tempfile::TempDir::new().unwrap();
        let config = DaemonConfig {
            storage_path: temp.path().to_path_buf(),
            root_mode: false,
        };
        let daemon = FaceAuthDaemon::new(config).unwrap();
        (temp, FaceAuthInterface::new(daemon))
    }

    /// Same as `test_interface()`, but with a caller-supplied `CameraManager`
    /// (typically `CameraManager::for_test()` with fake detector/extractor
    /// implementations) instead of a real one — lets register_face/verify's
    /// happy-path branches be exercised without real hardware or ONNX.
    fn test_interface_with_camera(
        camera: crate::camera::CameraManager,
    ) -> (tempfile::TempDir, FaceAuthInterface) {
        let temp = tempfile::TempDir::new().unwrap();
        let config = DaemonConfig {
            storage_path: temp.path().to_path_buf(),
            root_mode: false,
        };
        let daemon = FaceAuthDaemon::new_for_test(config, camera).unwrap();
        (temp, FaceAuthInterface::new(daemon))
    }

    /// `check_user_permission` compares against the *real* process UID, not
    /// anything injectable — so "acting on your own UID" in a test means
    /// this, and "a different UID" means anything else.
    fn my_uid() -> u32 {
        unsafe { libc::getuid() }
    }

    fn not_my_uid() -> u32 {
        let uid = my_uid();
        if uid == 1 {
            2
        } else {
            1
        }
    }

    #[tokio::test]
    async fn test_ping_returns_pong() {
        let (_temp, iface) = test_interface();
        assert_eq!(iface.ping().await.unwrap(), "pong");
    }

    #[tokio::test]
    async fn test_version_property_matches_crate_version() {
        let (_temp, iface) = test_interface();
        assert_eq!(iface.version(), env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_storage_path_property_matches_config() {
        let temp = tempfile::TempDir::new().unwrap();
        let config = DaemonConfig {
            storage_path: temp.path().to_path_buf(),
            root_mode: false,
        };
        let daemon = FaceAuthDaemon::new(config).unwrap();
        let iface = FaceAuthInterface::new(daemon);
        assert_eq!(iface.storage_path(), temp.path().to_string_lossy());
    }

    #[tokio::test]
    async fn test_root_mode_property_reflects_config() {
        let (_temp, iface) = test_interface();
        assert!(!iface.root_mode());
    }

    #[tokio::test]
    async fn test_camera_available_property_does_not_panic_without_hardware() {
        let (_temp, iface) = test_interface();
        // Whether it's true/false depends on the test machine's actual
        // camera inventory — just exercise the try_read/fallback path.
        let _ = iface.camera_available();
    }

    #[tokio::test]
    async fn test_camera_info_returns_well_formed_json() {
        let (_temp, iface) = test_interface();
        let json = iface.camera_info().await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("has_ir").is_some());
    }

    #[tokio::test]
    async fn test_register_face_rejects_malformed_json() {
        let (_temp, iface) = test_interface();
        assert!(iface.register_face("not json").await.is_err());
    }

    #[tokio::test]
    async fn test_delete_face_rejects_malformed_json() {
        let (_temp, iface) = test_interface();
        assert!(iface.delete_face("not json").await.is_err());
    }

    #[tokio::test]
    async fn test_verify_rejects_malformed_json() {
        let (_temp, iface) = test_interface();
        assert!(iface.verify("not json").await.is_err());
    }

    // Permission-denied paths: these all return before ever touching the
    // camera/detector, so — unlike a happy-path register_face/verify call —
    // they're safe to exercise without real hardware or an ONNX model.

    #[tokio::test]
    async fn test_register_face_rejects_a_different_users_uid() {
        if my_uid() == 0 {
            return; // root can act on any UID; the check is a no-op then
        }
        let (_temp, iface) = test_interface();
        let request = RegisterFaceRequest {
            user_id: not_my_uid(),
            context: "test".to_string(),
            timeout_ms: 100,
            num_samples: 1,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(iface.register_face(&json).await.is_err());
    }

    #[tokio::test]
    async fn test_delete_face_rejects_a_different_users_uid() {
        if my_uid() == 0 {
            return;
        }
        let (_temp, iface) = test_interface();
        let request = DeleteFaceRequest {
            user_id: not_my_uid(),
            face_id: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(iface.delete_face(&json).await.is_err());
    }

    #[tokio::test]
    async fn test_verify_rejects_a_different_users_uid() {
        if my_uid() == 0 {
            return;
        }
        let (_temp, iface) = test_interface();
        let request = VerifyRequest {
            user_id: not_my_uid(),
            context: "test".to_string(),
            timeout_ms: 100,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(iface.verify(&json).await.is_err());
    }

    // Happy paths that never touch the camera/detector at all (list/delete
    // are pure storage operations) — safe to run for real.

    #[tokio::test]
    async fn test_list_faces_for_a_fresh_user_returns_an_empty_json_array() {
        let (_temp, iface) = test_interface();
        assert_eq!(iface.list_faces(my_uid()).await.unwrap(), "[]");
    }

    #[tokio::test]
    async fn test_delete_all_faces_on_a_fresh_user_succeeds() {
        let (_temp, iface) = test_interface();
        let request = DeleteFaceRequest {
            user_id: my_uid(),
            face_id: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(iface.delete_face(&json).await.is_ok());
    }

    // Happy paths for register_face/verify — previously untested here since
    // they need a camera, and this project has already hit a real ORT-
    // dylib-load deadlock hazard from a similar shortcut once (see
    // hello_face_core::scrfd_detector's test comments). CameraManager::for_test()
    // sidesteps that entirely: no ONNX, no model files, no real device.

    fn fake_camera(
        detector: crate::test_support::FakeDetector,
        extractor: crate::test_support::FakeExtractor,
    ) -> (tempfile::TempDir, crate::camera::CameraManager) {
        let dir = tempfile::tempdir().unwrap();
        let rgb_device = dir
            .path()
            .join("no-camera-here")
            .to_string_lossy()
            .into_owned();
        let lock_path = dir.path().join("camera.lock");
        let camera = crate::camera::CameraManager::for_test(
            rgb_device,
            lock_path,
            Box::new(detector),
            Box::new(extractor),
        );
        (dir, camera)
    }

    #[tokio::test]
    async fn test_register_face_rejects_when_enrollment_not_authorized() {
        let (_cam_dir, camera) = fake_camera(
            crate::test_support::FakeDetector::always_detects(
                crate::test_support::default_face_region(640, 480),
            ),
            crate::test_support::FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );
        let temp = tempfile::TempDir::new().unwrap();
        let config = DaemonConfig {
            storage_path: temp.path().to_path_buf(),
            root_mode: false,
        };
        let daemon = FaceAuthDaemon::new_for_test(config, camera)
            .unwrap()
            .with_enrollment_authorizer(crate::authz::EnrollmentAuthorizer::DenyAll);
        let iface = FaceAuthInterface::new(daemon);

        let request = RegisterFaceRequest {
            user_id: my_uid(),
            context: "test".to_string(),
            timeout_ms: 1000,
            num_samples: 1,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(iface.register_face(&json).await.is_err());
    }

    #[tokio::test]
    async fn test_delete_face_rejects_when_enrollment_not_authorized() {
        let temp = tempfile::TempDir::new().unwrap();
        let config = DaemonConfig {
            storage_path: temp.path().to_path_buf(),
            root_mode: false,
        };
        let daemon = FaceAuthDaemon::new(config)
            .unwrap()
            .with_enrollment_authorizer(crate::authz::EnrollmentAuthorizer::DenyAll);
        let iface = FaceAuthInterface::new(daemon);

        let request = DeleteFaceRequest {
            user_id: my_uid(),
            face_id: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(iface.delete_face(&json).await.is_err());
    }

    #[tokio::test]
    async fn test_register_face_happy_path_returns_well_formed_response_json() {
        let (_cam_dir, camera) = fake_camera(
            crate::test_support::FakeDetector::always_detects(
                crate::test_support::default_face_region(640, 480),
            ),
            crate::test_support::FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );
        let (_temp, iface) = test_interface_with_camera(camera);

        let request = RegisterFaceRequest {
            user_id: my_uid(),
            context: "test".to_string(),
            timeout_ms: 1000,
            num_samples: 1,
        };
        let json = serde_json::to_string(&request).unwrap();
        let response_json = iface.register_face(&json).await.unwrap();
        let response: crate::dbus_interface::RegisterFaceResponse =
            serde_json::from_str(&response_json).unwrap();

        assert!(!response.face_id.is_empty());
        assert!(response.quality_score > 0.0);
    }

    #[tokio::test]
    async fn test_register_face_happy_path_then_list_faces_shows_the_new_record() {
        let (_cam_dir, camera) = fake_camera(
            crate::test_support::FakeDetector::always_detects(
                crate::test_support::default_face_region(640, 480),
            ),
            crate::test_support::FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );
        let (_temp, iface) = test_interface_with_camera(camera);
        let uid = my_uid();

        let request = RegisterFaceRequest {
            user_id: uid,
            context: "test".to_string(),
            timeout_ms: 1000,
            num_samples: 1,
        };
        let json = serde_json::to_string(&request).unwrap();
        let response_json = iface.register_face(&json).await.unwrap();
        let response: crate::dbus_interface::RegisterFaceResponse =
            serde_json::from_str(&response_json).unwrap();

        let faces_json = iface.list_faces(uid).await.unwrap();
        let faces: Vec<crate::FaceRecord> = serde_json::from_str(&faces_json).unwrap();
        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].face_id, response.face_id);
    }

    #[tokio::test]
    async fn test_verify_no_enrollment_returns_well_formed_json() {
        let (_cam_dir, camera) = fake_camera(
            crate::test_support::FakeDetector::always_detects(
                crate::test_support::default_face_region(640, 480),
            ),
            crate::test_support::FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );
        let (_temp, iface) = test_interface_with_camera(camera);

        let request = VerifyRequest {
            user_id: my_uid(),
            context: "test".to_string(),
            timeout_ms: 100,
        };
        let json = serde_json::to_string(&request).unwrap();
        let result_json = iface.verify(&json).await.unwrap();
        let result: crate::dbus_interface::VerifyResult =
            serde_json::from_str(&result_json).unwrap();

        assert!(matches!(
            result,
            crate::dbus_interface::VerifyResult::NoEnrollment
        ));
    }

    #[tokio::test]
    async fn test_verify_no_face_detected_after_enrollment_returns_well_formed_json() {
        // Same asymmetry as lib.rs's equivalent test: capture_until never
        // invokes its callback against an unavailable device, so a fresh
        // enrollment followed by verify deterministically lands on
        // NoFaceDetected rather than Success.
        let (_cam_dir, camera) = fake_camera(
            crate::test_support::FakeDetector::always_detects(
                crate::test_support::default_face_region(640, 480),
            ),
            crate::test_support::FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );
        let (_temp, iface) = test_interface_with_camera(camera);
        let uid = my_uid();

        let register_request = RegisterFaceRequest {
            user_id: uid,
            context: "test".to_string(),
            timeout_ms: 1000,
            num_samples: 1,
        };
        iface
            .register_face(&serde_json::to_string(&register_request).unwrap())
            .await
            .unwrap();

        let verify_request = VerifyRequest {
            user_id: uid,
            context: "test".to_string(),
            timeout_ms: 100,
        };
        let result_json = iface
            .verify(&serde_json::to_string(&verify_request).unwrap())
            .await
            .unwrap();
        let result: crate::dbus_interface::VerifyResult =
            serde_json::from_str(&result_json).unwrap();

        assert!(matches!(
            result,
            crate::dbus_interface::VerifyResult::NoFaceDetected
        ));
    }
}
