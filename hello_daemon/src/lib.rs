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

pub mod camera;
pub mod capture_stream;
pub mod dbus;
pub mod dbus_interface;
pub mod dbus_signals;
pub mod matcher;
pub mod pam_helper;
pub mod preview;
pub mod screenlock;
pub mod storage;

use camera::CameraManager;
use dbus_interface::{DeleteFaceRequest, RegisterFaceRequest, VerifyRequest, VerifyResult};
use matcher::FaceMatcher;
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

    /// Current UID if in user mode
    pub current_uid: Option<u32>,

    /// Default similarity threshold
    pub default_similarity_threshold: f32,

    /// Enable verbose logging
    pub debug: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let storage_path = if unsafe { libc::getuid() } == 0 {
            // Root mode: /var/lib/linux-hello/
            PathBuf::from("/var/lib/linux-hello")
        } else {
            // User mode: ~/.local/share/linux-hello/
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".local/share/linux-hello")
        };

        Self {
            storage_path,
            root_mode: unsafe { libc::getuid() } == 0,
            current_uid: None,
            default_similarity_threshold: 0.6,
            debug: false,
        }
    }
}

/// Metadata for a registered face
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FaceRecord {
    /// Unique ID
    pub face_id: String,

    /// Owning UID
    pub user_id: u32,

    /// Serialized embedding (JSON)
    pub embedding_json: String,

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
}

impl FaceAuthDaemon {
    pub fn new(config: DaemonConfig) -> Result<Self, DaemonError> {
        // Create the storage
        let storage = FaceStorage::new(&config.storage_path)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;

        // Create the camera manager
        let camera = CameraManager::new(5000); // 5s default timeout

        // Create the matcher with the configured threshold
        let matcher = FaceMatcher::new(config.default_similarity_threshold);

        info!("Daemon created with config: {:?}", config);

        Ok(Self {
            config,
            storage: Arc::new(storage),
            camera: Arc::new(camera),
            matcher: Arc::new(matcher),
        })
    }

    pub async fn register_face(&self, request: RegisterFaceRequest) -> Result<String, DaemonError> {
        // Check permissions
        self.check_user_permission(request.user_id)?;

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
            embedding_json: serde_json::to_string(&embedding.vector)?,
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
        verify_with_storage(&self.storage, &self.camera, &self.matcher, &request).await
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
    matcher: &FaceMatcher,
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

    // Capture 5 frames and take the best score
    const NUM_VERIFY_FRAMES: u32 = 5;
    let capture = camera
        .capture_frames(NUM_VERIFY_FRAMES, request.timeout_ms)
        .await
        .map_err(|e| DaemonError::CameraError(e.to_string()))?;

    // Filter the valid frames (with a detected face)
    let valid_probes: Vec<_> = capture
        .embeddings
        .iter()
        .filter(|e| !e.vector.is_empty() && e.metadata.quality_score > 0.0)
        .collect();

    if valid_probes.is_empty() {
        info!(
            "No face detected in the {} verification frames",
            NUM_VERIFY_FRAMES
        );
        return Ok(VerifyResult::NoFaceDetected);
    }

    info!(
        "{}/{} valid frames for verification",
        valid_probes.len(),
        NUM_VERIFY_FRAMES
    );

    // Load the stored embeddings
    let mut stored_embeddings = std::collections::HashMap::new();
    for face in &faces {
        let embedding = storage
            .load_face_embedding(request.user_id, &face.face_id)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;
        stored_embeddings.insert(face.face_id.clone(), embedding);
    }

    // Match each valid frame, keep the best result
    let best_result = valid_probes
        .iter()
        .map(|probe| {
            matcher.match_with_liveness(
                probe,
                &stored_embeddings,
                &request.context,
                capture.ir_liveness,
            )
        })
        .max_by(|a, b| {
            a.best_score
                .partial_cmp(&b.best_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap(); // safe: valid_probes is non-empty

    if best_result.matched {
        Ok(VerifyResult::Success {
            face_id: best_result.face_id.unwrap(),
            similarity_score: best_result.best_score,
        })
    } else if best_result.best_score > 0.0 {
        Ok(VerifyResult::NoMatch {
            best_score: best_result.best_score,
            threshold: best_result.threshold,
        })
    } else {
        Ok(VerifyResult::NoFaceDetected)
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
            embedding_json: "[]".to_string(),
            quality_score: 0.95,
            registered_at: 0,
            context: "login".to_string(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let restored: FaceRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.face_id, restored.face_id);
    }
}
