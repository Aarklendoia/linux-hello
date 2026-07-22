//! Implementation of the facial recognition daemon
//!
//! Handles:
//! - Storage of face embeddings
//! - D-Bus interface for enrollment/verification
//! - Camera access
//! - Matching and scoring

use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

pub mod authz;
pub mod camera;
pub mod capture_stream;
pub mod dbus;
pub mod dbus_interface;
pub mod dbus_signals;
pub mod matcher;
pub mod pam_helper;
pub mod preview;
pub mod screenlock;
mod security_util;
pub mod storage;
#[cfg(test)]
mod test_support;

use authz::EnrollmentAuthorizer;
use camera::CameraManager;
use dbus_interface::{DeleteFaceRequest, RegisterFaceRequest, VerifyRequest, VerifyResult};
use matcher::{FaceMatcher, MatchResult};
use storage::FaceStorage;

/// Daemon errors
#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("User not found: {0}")]
    UserNotFound(u32),

    #[error("Face not found: {0}")]
    FaceNotFound(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Storage failed: {0}")]
    StorageError(String),

    #[error("D-Bus error: {0}")]
    DbusError(String),

    #[error("Camera: {0}")]
    CameraError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Daemon configuration
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Directory for storing embeddings
    pub storage_path: PathBuf,

    /// Root mode (true) or user mode (false)
    pub root_mode: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let storage_path = if unsafe { libc::getuid() } == 0 {
            // Root mode: /var/lib/linux-hello/
            PathBuf::from("/var/lib/linux-hello")
        } else {
            // User mode: $XDG_DATA_HOME/linux-hello, falling back to
            // ~/.local/share/linux-hello (dirs::data_dir()'s own fallback
            // chain), or "." if neither can be determined.
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("linux-hello")
        };

        Self {
            storage_path,
            root_mode: unsafe { libc::getuid() } == 0,
        }
    }
}

/// Metadata for a registered face.
///
/// Deliberately does NOT carry the embedding vector itself (that lives only
/// in the separate `<face_id>.embedding.json` file, see
/// `FaceStorage::save_face`/`load_face_embedding`): this struct is also what
/// `list_faces` returns verbatim over the session D-Bus bus, which has no
/// access-control policy of its own beyond "same UID as the daemon" — any
/// local process on that bus can call it. A raw face-recognition embedding is
/// reconstructable/comparable biometric data, so it must never round-trip
/// through that channel, even though the D-Bus caller is already the same
/// user the data belongs to.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FaceRecord {
    /// Unique ID
    pub face_id: String,

    /// Owning UID
    pub user_id: u32,

    /// Quality score
    pub quality_score: f32,

    /// Enrollment timestamp
    pub registered_at: u64,

    /// Enrollment context
    pub context: String,
}

/// Implementation of the D-Bus service with all components
pub struct FaceAuthDaemon {
    config: DaemonConfig,
    storage: Arc<FaceStorage>,
    camera: Arc<CameraManager>,
    matcher: Arc<FaceMatcher>,
    /// Gate on register_face/delete_face — see `authz` module docs. Defaults
    /// to `AllowAll` (no gate) here so every existing test constructing a
    /// daemon via `new`/`new_for_test` is unaffected; production wires the
    /// real check explicitly via `with_enrollment_authorizer` (see main.rs),
    /// the same opt-in pattern `new_with_connection`/`from_arc` already use
    /// for `FaceAuthInterface`'s signal emitter.
    enrollment_authorizer: EnrollmentAuthorizer,
}

impl FaceAuthDaemon {
    pub fn new(config: DaemonConfig) -> Result<Self, DaemonError> {
        // Create the storage
        let storage = FaceStorage::new(&config.storage_path)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;

        // Create the camera manager
        let camera = CameraManager::new(5000); // 5s default timeout

        // Create the matcher with the configured threshold
        let matcher = FaceMatcher::new();

        info!("Daemon created with config: {:?}", config);

        Ok(Self {
            config,
            storage: Arc::new(storage),
            camera: Arc::new(camera),
            matcher: Arc::new(matcher),
            #[cfg(test)]
            enrollment_authorizer: EnrollmentAuthorizer::AllowAll,
            #[cfg(not(test))]
            enrollment_authorizer: EnrollmentAuthorizer::from_env(),
        })
    }

    /// Same as `new()`, but takes a pre-built `CameraManager` instead of
    /// always constructing one via `CameraManager::new(5000)` — lets tests
    /// wire in a `CameraManager::for_test()` with fake detector/extractor
    /// implementations, all the way through to `FaceAuthInterface`.
    #[cfg(test)]
    pub(crate) fn new_for_test(
        config: DaemonConfig,
        camera: CameraManager,
    ) -> Result<Self, DaemonError> {
        let storage = FaceStorage::new(&config.storage_path)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;
        let matcher = FaceMatcher::new();
        Ok(Self {
            config,
            storage: Arc::new(storage),
            camera: Arc::new(camera),
            matcher: Arc::new(matcher),
            enrollment_authorizer: EnrollmentAuthorizer::AllowAll,
        })
    }

    /// Swaps in a different enrollment authorizer — production (`main.rs`)
    /// uses this to wire up the real polkit-backed check; tests that
    /// specifically exercise the gate use it to wire `DenyAll`.
    #[cfg(test)]
    pub(crate) fn with_enrollment_authorizer(mut self, authorizer: EnrollmentAuthorizer) -> Self {
        self.enrollment_authorizer = authorizer;
        self
    }

    pub async fn register_face(&self, request: RegisterFaceRequest) -> Result<String, DaemonError> {
        // Check permissions
        self.check_user_permission(request.user_id)?;

        // A successfully enrolled face unlocks sudo (via pam_linux_hello.so),
        // so this can't be a same-UID-only decision like the check above —
        // see the `authz` module docs for why this needs a fresh, interactive
        // re-authentication instead.
        if !self
            .enrollment_authorizer
            .authorize(authz::MANAGE_FACES_ACTION)
            .await
        {
            return Err(DaemonError::AccessDenied(
                "Enrollment was not authorized".to_string(),
            ));
        }

        info!(
            "Registering face for user_id={}, context={}",
            request.user_id, request.context
        );

        // Capture frames
        let capture = self
            .camera
            .capture_frames(request.num_samples, request.timeout_ms)
            .await
            .map_err(|e| DaemonError::CameraError(e.to_string()))?;

        // Average all valid embeddings, then normalize.
        // An average embedding represents the "center" of the user's face
        // and gives more stable similarity scores during authentication.
        let valid: Vec<_> = capture
            .embeddings
            .iter()
            .filter(|e| !e.vector.is_empty() && e.metadata.quality_score > 0.0)
            .collect();
        if valid.is_empty() {
            return Err(DaemonError::CameraError("No face detected".to_string()));
        }
        let dim = valid[0].vector.len();
        let mut avg = vec![0.0f32; dim];
        for e in &valid {
            for (a, v) in avg.iter_mut().zip(e.vector.iter()) {
                *a += v;
            }
        }
        let n = valid.len() as f32;
        for a in avg.iter_mut() {
            *a /= n;
        }
        // Normalize (required for cosine similarity)
        let norm: f32 = avg.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for a in avg.iter_mut() {
                *a /= norm;
            }
        }
        info!(
            "Enrollment: {} frames averaged into 1 embedding",
            valid.len()
        );
        let best = valid
            .iter()
            .max_by(|a, b| {
                a.metadata
                    .quality_score
                    .partial_cmp(&b.metadata.quality_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        let embedding = &hello_face_core::Embedding {
            vector: avg,
            metadata: best.metadata.clone(),
        };

        // Generate a unique ID for this face
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let face_id = format!("face_{}_{}", request.user_id, now);

        // Create the record
        let record = FaceRecord {
            face_id: face_id.clone(),
            user_id: request.user_id,
            quality_score: capture.quality_score,
            registered_at: now,
            context: request.context.clone(),
        };

        // Save
        self.storage
            .save_face(&record, embedding)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;

        info!("Face registered: face_id={}", face_id);

        // Return the response JSON
        let response = dbus_interface::RegisterFaceResponse {
            face_id,
            registered_at: now,
            quality_score: capture.quality_score,
        };

        Ok(serde_json::to_string(&response)?)
    }

    pub async fn delete_face(&self, request: DeleteFaceRequest) -> Result<(), DaemonError> {
        self.check_user_permission(request.user_id)?;

        // Same re-authentication requirement as register_face — see its
        // comment and the `authz` module docs. Deleting a face isn't itself
        // a privilege gain, but combined with an unauthorized register_face
        // it lets an attacker silently replace the legitimate user's sudo
        // credential with their own.
        if !self
            .enrollment_authorizer
            .authorize(authz::MANAGE_FACES_ACTION)
            .await
        {
            return Err(DaemonError::AccessDenied(
                "Enrollment change was not authorized".to_string(),
            ));
        }

        info!(
            "Deleting face for user_id={}, face_id={:?}",
            request.user_id, request.face_id
        );

        match request.face_id {
            Some(face_id) => {
                self.storage
                    .delete_face(request.user_id, &face_id)
                    .map_err(|e| DaemonError::StorageError(e.to_string()))?;
            }
            None => {
                self.storage
                    .delete_all_faces(request.user_id)
                    .map_err(|e| DaemonError::StorageError(e.to_string()))?;
            }
        }

        Ok(())
    }

    pub async fn verify(&self, request: VerifyRequest) -> Result<VerifyResult, DaemonError> {
        // Check permissions
        self.check_user_permission(request.user_id)?;
        verify_with_storage(
            &self.storage,
            &self.camera,
            Arc::clone(&self.matcher),
            &request,
        )
        .await
    }

    pub async fn list_faces(&self, user_id: u32) -> Result<String, DaemonError> {
        self.check_user_permission(user_id)?;

        let faces = self
            .storage
            .list_user_faces(user_id)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;

        Ok(serde_json::to_string(&faces)?)
    }

    /// Check that the current user has permission to access this UID
    fn check_user_permission(&self, target_uid: u32) -> Result<(), DaemonError> {
        let current_uid = unsafe { libc::getuid() };

        // Root can do anything
        if current_uid == 0 {
            return Ok(());
        }

        // A user can access their own face
        if current_uid == target_uid {
            return Ok(());
        }

        Err(DaemonError::AccessDenied(format!(
            "UID {} cannot access UID {}",
            current_uid, target_uid
        )))
    }

    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }

    pub fn is_camera_available(&self) -> bool {
        self.camera.is_available()
    }

    pub fn camera_manager(&self) -> &CameraManager {
        &self.camera
    }
}

/// Accumulated state across the frames of one `verify_with_storage` capture
/// attempt.
#[derive(Default)]
struct VerifyLoopState {
    consecutive: u32,
    any_face_detected: bool,
    /// Highest-scoring frame seen so far, matched or not — used to report a
    /// helpful `NoMatch` score/threshold if nothing ever hits the
    /// consecutive-match requirement.
    best_result: Option<MatchResult>,
    /// Set exactly once, on the frame that completes
    /// `required_consecutive` — guaranteed `matched` with a `face_id`,
    /// unlike `best_result` above.
    success_result: Option<MatchResult>,
}

/// Folds one frame's `MatchResult` into `state` and returns whether the
/// capture loop should stop now (`required_consecutive` matching frames
/// reached in a row). Pure/deterministic — exercised directly in unit tests
/// below without needing a real camera.
fn record_frame_result(
    state: &mut VerifyLoopState,
    result: MatchResult,
    required_consecutive: u32,
) -> bool {
    state.any_face_detected = true;

    let is_better = state
        .best_result
        .as_ref()
        .map(|b| result.best_score > b.best_score)
        .unwrap_or(true);
    if is_better {
        state.best_result = Some(result.clone());
    }

    if result.matched {
        state.consecutive += 1;
        if state.consecutive >= required_consecutive && state.success_result.is_none() {
            state.success_result = Some(result);
        }
    } else {
        state.consecutive = 0;
    }

    state.success_result.is_some()
}

/// Verify a face against a given storage/camera/matcher, independent of any
/// particular `FaceAuthDaemon` instance.
///
/// Extracted from `FaceAuthDaemon::verify()` so the SDDM system listener
/// (`hello-daemon-system`, see `pam_helper.rs`) can reuse the exact same
/// capture/match logic against a *different* user's storage, resolved
/// per-request from their home directory — `FaceAuthDaemon` itself is fixed
/// to one `base_path` for its whole lifetime and has no per-call storage
/// swap. Permission checks (`check_user_permission`) are the caller's
/// responsibility, not this function's: `FaceAuthDaemon::verify()` checks
/// against the daemon process's own UID, while the system listener checks
/// the *socket peer's* credentials instead (see `pam_helper.rs`) — the two
/// have different trust models, so the check can't live here.
pub async fn verify_with_storage(
    storage: &FaceStorage,
    camera: &CameraManager,
    matcher: Arc<FaceMatcher>,
    request: &VerifyRequest,
) -> Result<VerifyResult, DaemonError> {
    info!(
        "Verifying for user_id={}, context={}",
        request.user_id, request.context
    );

    // Load the registered faces
    let faces = storage
        .list_user_faces(request.user_id)
        .map_err(|e| DaemonError::StorageError(e.to_string()))?;

    if faces.is_empty() {
        info!("No face registered for user_id={}", request.user_id);
        return Ok(VerifyResult::NoEnrollment);
    }

    // Load the stored embeddings
    let mut stored_embeddings = std::collections::HashMap::new();
    for face in &faces {
        let embedding = storage
            .load_face_embedding(request.user_id, &face.face_id)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;
        stored_embeddings.insert(face.face_id.clone(), embedding);
    }
    let stored_embeddings = Arc::new(stored_embeddings);

    // Camera stays engaged (no on/off blink) and keeps trying for the whole
    // request.timeout_ms window instead of a fixed quick burst — gives the
    // user real time to notice the prompt and turn toward the camera.
    // Requiring 2 consecutive matching (+ liveness-passing) frames before
    // declaring Success offsets the fact that a long window gives many more
    // independent chances at a single lucky false-accept than the old fixed
    // 5-frame batch did; a genuine match reliably produces 2 consecutive
    // good frames within well under a second at the observed ~220ms/frame
    // capture rate.
    const REQUIRED_CONSECUTIVE_MATCHES: u32 = 2;

    let state = Arc::new(std::sync::Mutex::new(VerifyLoopState::default()));

    let context = request.context.clone();
    let state_clone = Arc::clone(&state);
    let stored_clone = Arc::clone(&stored_embeddings);

    camera
        .capture_until(
            request.timeout_ms,
            move |embedding, ir_liveness, rgb_liveness| {
                let result = matcher.match_with_liveness(
                    &embedding,
                    &stored_clone,
                    &context,
                    ir_liveness,
                    rgb_liveness,
                );
                let mut s = state_clone.lock().unwrap();
                record_frame_result(&mut s, result, REQUIRED_CONSECUTIVE_MATCHES)
            },
        )
        .await
        .map_err(|e| DaemonError::CameraError(e.to_string()))?;

    let final_state = state.lock().unwrap();

    if let Some(success) = &final_state.success_result {
        info!(
            "Face recognized after {} consecutive matching frames (score={:.3})",
            REQUIRED_CONSECUTIVE_MATCHES, success.best_score
        );
        return Ok(VerifyResult::Success {
            face_id: success.face_id.clone().unwrap_or_default(),
            similarity_score: success.best_score,
        });
    }

    if !final_state.any_face_detected {
        info!("No face detected within the verification window");
        return Ok(VerifyResult::NoFaceDetected);
    }

    match &final_state.best_result {
        Some(best) if best.best_score > 0.0 => Ok(VerifyResult::NoMatch {
            best_score: best.best_score,
            threshold: best.threshold,
        }),
        _ => Ok(VerifyResult::NoFaceDetected),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert!(!config.storage_path.as_os_str().is_empty());
    }

    #[test]
    fn test_face_record_serialization() {
        let record = FaceRecord {
            face_id: "face_1".to_string(),
            user_id: 1000,
            quality_score: 0.95,
            registered_at: 0,
            context: "login".to_string(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let restored: FaceRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.face_id, restored.face_id);
    }

    fn match_result(matched: bool, score: f32, face_id: &str) -> MatchResult {
        MatchResult {
            face_id: if matched {
                Some(face_id.to_string())
            } else {
                None
            },
            best_score: score,
            threshold: 0.6,
            all_scores: std::collections::HashMap::new(),
            matched,
        }
    }

    #[test]
    fn test_record_frame_result_stops_after_required_consecutive_matches() {
        let mut state = VerifyLoopState::default();

        // First matching frame: not enough yet (need 2 in a row).
        let stop = record_frame_result(&mut state, match_result(true, 0.7, "face_1"), 2);
        assert!(!stop);
        assert_eq!(state.consecutive, 1);
        assert!(state.success_result.is_none());

        // Second consecutive matching frame: now it should stop.
        let stop = record_frame_result(&mut state, match_result(true, 0.72, "face_1"), 2);
        assert!(stop);
        assert_eq!(state.consecutive, 2);
        assert!(state.success_result.is_some());
    }

    #[test]
    fn test_record_frame_result_resets_streak_on_non_match() {
        let mut state = VerifyLoopState::default();

        record_frame_result(&mut state, match_result(true, 0.7, "face_1"), 2);
        assert_eq!(state.consecutive, 1);

        // A non-matching frame in between must reset the streak — a single
        // lucky frame surrounded by misses must not grant access.
        let stop = record_frame_result(&mut state, match_result(false, 0.2, "face_1"), 2);
        assert!(!stop);
        assert_eq!(state.consecutive, 0);

        // Two fresh consecutive matches after the reset should still work.
        record_frame_result(&mut state, match_result(true, 0.7, "face_1"), 2);
        let stop = record_frame_result(&mut state, match_result(true, 0.71, "face_1"), 2);
        assert!(stop);
    }

    #[test]
    fn test_record_frame_result_tracks_best_score_for_reporting() {
        let mut state = VerifyLoopState::default();

        record_frame_result(&mut state, match_result(false, 0.3, "face_1"), 2);
        record_frame_result(&mut state, match_result(false, 0.5, "face_1"), 2);
        record_frame_result(&mut state, match_result(false, 0.1, "face_1"), 2);

        assert!(state.any_face_detected);
        assert!(state.success_result.is_none());
        assert_eq!(state.best_result.unwrap().best_score, 0.5);
    }

    #[test]
    fn test_record_frame_result_success_result_locked_in_once() {
        let mut state = VerifyLoopState::default();

        record_frame_result(&mut state, match_result(true, 0.6, "face_1"), 2);
        record_frame_result(&mut state, match_result(true, 0.6, "face_1"), 2);
        // Further frames after success must not overwrite the recorded
        // success_result (the loop is expected to have already stopped, but
        // this guards the state transition itself regardless).
        record_frame_result(&mut state, match_result(true, 0.99, "face_2"), 2);

        assert_eq!(state.success_result.unwrap().face_id.unwrap(), "face_1");
    }

    use crate::test_support::{blank_rgb_frame, default_face_region, FakeDetector, FakeExtractor};

    fn my_uid() -> u32 {
        unsafe { libc::getuid() }
    }

    fn test_config(storage_path: std::path::PathBuf) -> DaemonConfig {
        DaemonConfig {
            storage_path,
            root_mode: false,
        }
    }

    /// A `CameraManager` pointed at a device path that can't exist, with its
    /// own private lock file — matches camera::tests's own helper, but
    /// that one is private to camera.rs's test module.
    fn test_camera(
        detector: FakeDetector,
        extractor: FakeExtractor,
    ) -> (tempfile::TempDir, CameraManager) {
        let dir = tempfile::tempdir().unwrap();
        let rgb_device = dir
            .path()
            .join("no-camera-here")
            .to_string_lossy()
            .into_owned();
        let lock_path = dir.path().join("camera.lock");
        let camera = CameraManager::for_test(
            rgb_device,
            lock_path,
            Box::new(detector),
            Box::new(extractor),
        );
        (dir, camera)
    }

    #[tokio::test]
    async fn test_register_face_normalizes_a_single_repeated_embedding() {
        let storage_dir = tempfile::TempDir::new().unwrap();
        let (_cam_dir, camera) = test_camera(
            FakeDetector::always_detects(default_face_region(640, 480)),
            FakeExtractor::with_vector(vec![3.0, 4.0, 0.0], 0.8),
        );
        let daemon =
            FaceAuthDaemon::new_for_test(test_config(storage_dir.path().to_path_buf()), camera)
                .unwrap();

        let uid = my_uid();
        let response_json = daemon
            .register_face(RegisterFaceRequest {
                user_id: uid,
                context: "test".to_string(),
                timeout_ms: 1000,
                num_samples: 1,
            })
            .await
            .unwrap();
        let response: dbus_interface::RegisterFaceResponse =
            serde_json::from_str(&response_json).unwrap();

        // [3,4,0] normalized -> [0.6,0.8,0.0]
        let storage = storage::FaceStorage::new(storage_dir.path()).unwrap();
        let embedding = storage.load_face_embedding(uid, &response.face_id).unwrap();
        assert!((embedding.vector[0] - 0.6).abs() < 1e-5);
        assert!((embedding.vector[1] - 0.8).abs() < 1e-5);
        assert!(embedding.vector[2].abs() < 1e-5);
    }

    #[tokio::test]
    async fn test_register_face_averages_multiple_distinct_embeddings() {
        let storage_dir = tempfile::TempDir::new().unwrap();
        let (_cam_dir, camera) = test_camera(
            FakeDetector::always_detects(default_face_region(640, 480)),
            FakeExtractor::sequence(vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]], 0.8),
        );
        let daemon =
            FaceAuthDaemon::new_for_test(test_config(storage_dir.path().to_path_buf()), camera)
                .unwrap();

        let uid = my_uid();
        let response_json = daemon
            .register_face(RegisterFaceRequest {
                user_id: uid,
                context: "test".to_string(),
                timeout_ms: 1000,
                num_samples: 2,
            })
            .await
            .unwrap();
        let response: dbus_interface::RegisterFaceResponse =
            serde_json::from_str(&response_json).unwrap();

        // mean([1,0,0],[0,1,0]) = [0.5,0.5,0], normalized -> [0.7071,0.7071,0]
        let storage = storage::FaceStorage::new(storage_dir.path()).unwrap();
        let embedding = storage.load_face_embedding(uid, &response.face_id).unwrap();
        let expected = 1.0_f32 / 2.0_f32.sqrt();
        assert!((embedding.vector[0] - expected).abs() < 1e-4);
        assert!((embedding.vector[1] - expected).abs() < 1e-4);
        assert!(embedding.vector[2].abs() < 1e-5);
    }

    #[tokio::test]
    async fn test_register_face_fails_when_no_face_ever_detected() {
        let storage_dir = tempfile::TempDir::new().unwrap();
        let (_cam_dir, camera) = test_camera(
            FakeDetector::never_detects(),
            FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );
        let daemon =
            FaceAuthDaemon::new_for_test(test_config(storage_dir.path().to_path_buf()), camera)
                .unwrap();

        let result = daemon
            .register_face(RegisterFaceRequest {
                user_id: my_uid(),
                context: "test".to_string(),
                timeout_ms: 1000,
                num_samples: 2,
            })
            .await;

        let err = result.expect_err("no face should ever be detected");
        assert!(err.to_string().contains("No face detected"));
    }

    #[tokio::test]
    async fn test_verify_returns_no_enrollment_when_nothing_registered() {
        let storage_dir = tempfile::TempDir::new().unwrap();
        let (_cam_dir, camera) = test_camera(
            FakeDetector::always_detects(default_face_region(640, 480)),
            FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );
        let daemon =
            FaceAuthDaemon::new_for_test(test_config(storage_dir.path().to_path_buf()), camera)
                .unwrap();

        let result = daemon
            .verify(VerifyRequest {
                user_id: my_uid(),
                context: "test".to_string(),
                timeout_ms: 100,
            })
            .await
            .unwrap();

        assert!(matches!(result, VerifyResult::NoEnrollment));
    }

    #[tokio::test]
    async fn test_verify_returns_no_face_detected_after_enrollment_when_camera_yields_nothing() {
        // capture_until (verify's path) never invokes its callback at all
        // when the device can't be opened, unlike capture_frames
        // (register_face's path) which pads with stub frames — see
        // camera::tests::test_capture_until_with_unavailable_device_never_invokes_callback.
        // So a fresh enrollment followed by verify against the same fake
        // camera deterministically lands on NoFaceDetected.
        let storage_dir = tempfile::TempDir::new().unwrap();
        let (_cam_dir, camera) = test_camera(
            FakeDetector::always_detects(default_face_region(640, 480)),
            FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );
        let daemon =
            FaceAuthDaemon::new_for_test(test_config(storage_dir.path().to_path_buf()), camera)
                .unwrap();
        let uid = my_uid();

        daemon
            .register_face(RegisterFaceRequest {
                user_id: uid,
                context: "test".to_string(),
                timeout_ms: 1000,
                num_samples: 1,
            })
            .await
            .unwrap();

        let result = daemon
            .verify(VerifyRequest {
                user_id: uid,
                context: "test".to_string(),
                timeout_ms: 100,
            })
            .await
            .unwrap();

        assert!(matches!(result, VerifyResult::NoFaceDetected));
    }

    /// Chains score_frame -> matcher::match_with_liveness -> record_frame_result
    /// directly — exactly what capture_until's closure does internally —
    /// to exercise the actual Success/NoMatch consecutive-frame
    /// orchestration without needing frames to flow through a real or fake
    /// camera at all (capture_until can't help here: see the test above).
    #[test]
    fn test_verify_orchestration_succeeds_after_two_consecutive_matching_frames() {
        let detector = FakeDetector::always_detects(default_face_region(640, 480));
        let extractor = FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9);
        let frame = blank_rgb_frame(640, 480);

        let mut stored = std::collections::HashMap::new();
        stored.insert(
            "face_1".to_string(),
            hello_face_core::Embedding {
                vector: vec![1.0, 0.0, 0.0],
                metadata: hello_face_core::EmbeddingMetadata {
                    model: "test".to_string(),
                    model_version: "test".to_string(),
                    extracted_at: 0,
                    quality_score: 0.9,
                },
            },
        );
        let matcher = FaceMatcher::new();
        let mut state = VerifyLoopState::default();

        for _ in 0..2 {
            let (embedding, rgb_liveness) =
                camera::score_frame(&detector, &extractor, 0, &frame, 640, 480).unwrap();
            let result =
                matcher.match_with_liveness(&embedding, &stored, "test", None, rgb_liveness);
            record_frame_result(&mut state, result, 2);
        }

        let success = state
            .success_result
            .expect("2 consecutive matches should succeed");
        assert_eq!(success.face_id.unwrap(), "face_1");
    }

    #[test]
    fn test_verify_orchestration_reports_no_match_for_a_dissimilar_embedding() {
        let detector = FakeDetector::always_detects(default_face_region(640, 480));
        // Orthogonal to the stored [1,0,0] -> cosine similarity 0.0
        let extractor = FakeExtractor::with_vector(vec![0.0, 1.0, 0.0], 0.9);
        let frame = blank_rgb_frame(640, 480);

        let mut stored = std::collections::HashMap::new();
        stored.insert(
            "face_1".to_string(),
            hello_face_core::Embedding {
                vector: vec![1.0, 0.0, 0.0],
                metadata: hello_face_core::EmbeddingMetadata {
                    model: "test".to_string(),
                    model_version: "test".to_string(),
                    extracted_at: 0,
                    quality_score: 0.9,
                },
            },
        );
        let matcher = FaceMatcher::new();
        let mut state = VerifyLoopState::default();

        let (embedding, rgb_liveness) =
            camera::score_frame(&detector, &extractor, 0, &frame, 640, 480).unwrap();
        let result = matcher.match_with_liveness(&embedding, &stored, "test", None, rgb_liveness);
        record_frame_result(&mut state, result, 2);

        assert!(state.success_result.is_none());
        assert!(state.any_face_detected);
    }
}
