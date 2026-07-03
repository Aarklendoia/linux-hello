//! D-Bus API exposed by the daemon
//!
//! Interface: com.linuxhello.FaceAuth
//! Path: /com/linuxhello/FaceAuth

use serde::{Deserialize, Serialize};
use std::fmt;
use zbus::interface;

// ============================================================================
// Serializable request/response types
// ============================================================================

/// Face enrollment request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterFaceRequest {
    /// UID of the owning user
    pub user_id: u32,

    /// Context (login, sudo, screenlock, sddm, test)
    pub context: String,

    /// Max timeout in ms for the capture/enrollment
    pub timeout_ms: u64,

    /// Number of frames to capture and average
    pub num_samples: u32,
}

/// Successful enrollment response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterFaceResponse {
    /// Unique ID of the enrolled model
    pub face_id: String,

    /// Enrollment timestamp
    pub registered_at: u64,

    /// Average quality score of the embeddings
    pub quality_score: f32,
}

/// Face deletion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFaceRequest {
    /// User UID
    pub user_id: u32,

    /// ID of the face to delete (None = all)
    pub face_id: Option<String>,
}

/// Verification request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    /// UID of the user to authenticate
    pub user_id: u32,

    /// Context
    pub context: String,

    /// Max timeout in ms
    pub timeout_ms: u64,
}

/// Verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerifyResult {
    /// Success
    Success {
        face_id: String,
        similarity_score: f32,
    },

    /// No face detected
    NoFaceDetected,

    /// Face detected but not recognized
    NoMatch { best_score: f32, threshold: f32 },

    /// No enrolled models
    NoEnrollment,

    /// User cancelled
    Cancelled,

    /// Error
    Error { message: String },
}

impl fmt::Display for VerifyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyResult::Success {
                face_id,
                similarity_score,
            } => {
                write!(f, "Success ({}): {:.2}", face_id, similarity_score)
            }
            VerifyResult::NoFaceDetected => write!(f, "No face"),
            VerifyResult::NoMatch {
                best_score,
                threshold,
            } => {
                write!(f, "Not recognized: {:.2} < {:.2}", best_score, threshold)
            }
            VerifyResult::NoEnrollment => write!(f, "No enrollment"),
            VerifyResult::Cancelled => write!(f, "Cancelled"),
            VerifyResult::Error { message } => write!(f, "Error: {}", message),
        }
    }
}

// ============================================================================
// D-Bus interface
// ============================================================================

/// D-Bus interface for facial authentication
pub struct FaceAuthService {
    // To be filled in with the impl in lib.rs
}

#[interface(name = "com.linuxhello.FaceAuth")]
impl FaceAuthService {
    /// Register a new face for a user
    ///
    /// # Arguments
    /// * `request` - JSON of RegisterFaceRequest
    ///
    /// # Returns
    /// JSON of RegisterFaceResponse or error
    pub async fn register_face(&self, _request: &str) -> zbus::fdo::Result<String> {
        // Impl in lib.rs
        Err(zbus::fdo::Error::Failed("Not implemented".to_string()))
    }

    /// Delete one or all faces
    ///
    /// # Arguments
    /// * `request` - JSON of DeleteFaceRequest
    pub async fn delete_face(&self, _request: &str) -> zbus::fdo::Result<()> {
        // Impl in lib.rs
        Err(zbus::fdo::Error::Failed("Not implemented".to_string()))
    }

    /// Verify a user's identity
    ///
    /// # Arguments
    /// * `request` - JSON of VerifyRequest
    ///
    /// # Returns
    /// JSON of VerifyResult
    pub async fn verify(&self, _request: &str) -> zbus::fdo::Result<String> {
        // Impl in lib.rs
        Err(zbus::fdo::Error::Failed("Not implemented".to_string()))
    }

    /// List the faces registered for a user
    ///
    /// # Arguments
    /// * `user_id` - UID
    ///
    /// # Returns
    /// JSON array of face_ids and metadata
    pub async fn list_faces(&self, _user_id: u32) -> zbus::fdo::Result<String> {
        // Impl in lib.rs
        Err(zbus::fdo::Error::Failed("Not implemented".to_string()))
    }

    /// Check that the daemon is operational
    pub async fn ping(&self) -> zbus::fdo::Result<String> {
        Ok("pong".to_string())
    }

    /// Daemon version
    #[zbus(property)]
    pub async fn version(&self) -> zbus::fdo::Result<String> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }

    /// Check whether a camera is available
    #[zbus(property)]
    pub async fn camera_available(&self) -> zbus::fdo::Result<bool> {
        // To be implemented with real detection
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_result_display() {
        let result = VerifyResult::Success {
            face_id: "face_1".to_string(),
            similarity_score: 0.87,
        };
        assert!(result.to_string().contains("0.87"));
    }

    #[test]
    fn test_register_request_serialization() {
        let req = RegisterFaceRequest {
            user_id: 1000,
            context: "login".to_string(),
            timeout_ms: 5000,
            num_samples: 3,
        };
        let json = serde_json::to_string(&req).unwrap();
        let restored: RegisterFaceRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.user_id, restored.user_id);
    }
}
