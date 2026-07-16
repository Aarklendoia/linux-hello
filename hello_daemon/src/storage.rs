//! Persistent storage management for face embeddings
//!
//! - SQLite for metadata (user_id, face_id, quality_score, registered_at)
//! - JSON files for embeddings (for flexibility)

use crate::{DaemonError, FaceRecord};
use hello_face_core::Embedding;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Face storage manager
pub struct FaceStorage {
    /// Root storage directory
    base_path: PathBuf,

    /// Path to the SQLite DB
    #[allow(dead_code)]
    db_path: PathBuf,
}

impl FaceStorage {
    /// Create a new storage manager
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self, DaemonError> {
        let base_path = base_path.as_ref().to_path_buf();

        // Create the directory structure
        std::fs::create_dir_all(&base_path)
            .map_err(|e| DaemonError::StorageError(format!("Directory creation failed: {}", e)))?;

        let db_path = base_path.join("faces.db");

        let storage = Self { base_path, db_path };

        // Initialize the DB if it doesn't exist
        storage.init_db()?;

        Ok(storage)
    }

    /// Open an existing storage directory without creating anything.
    ///
    /// Returns `Ok(None)` if `base_path` doesn't exist yet (e.g. the user has
    /// never enrolled). Unlike `new()`, this never calls `create_dir_all` —
    /// used by the SDDM system listener, which reads an arbitrary,
    /// not-yet-authenticated user's home directory and must never create
    /// directories there as a side effect of a failed or in-progress login
    /// attempt.
    pub fn open_read_only(base_path: impl AsRef<Path>) -> Result<Option<Self>, DaemonError> {
        let base_path = base_path.as_ref().to_path_buf();
        if !base_path.is_dir() {
            return Ok(None);
        }
        let db_path = base_path.join("faces.db");
        Ok(Some(Self { base_path, db_path }))
    }

    /// Initialize the SQLite structure
    fn init_db(&self) -> Result<(), DaemonError> {
        // Create the embeddings directory
        let embeddings_dir = self.base_path.join("embeddings");
        std::fs::create_dir_all(&embeddings_dir)
            .map_err(|e| DaemonError::StorageError(format!("Embeddings dir creation: {}", e)))?;

        // For now we use JSON files
        // Migration to SQLite will happen later with sqlx async
        info!("Storage initialized at: {}", self.base_path.display());

        Ok(())
    }

    /// Save a newly registered face
    pub fn save_face(&self, record: &FaceRecord, embedding: &Embedding) -> Result<(), DaemonError> {
        // Check permissions of the user directory
        let user_dir = self.user_dir(record.user_id)?;
        std::fs::create_dir_all(&user_dir)
            .map_err(|e| DaemonError::StorageError(format!("User dir creation: {}", e)))?;

        // Save metadata to a JSON file
        let metadata_path = user_dir.join(format!("{}.meta.json", record.face_id));
        let metadata_json =
            serde_json::to_string_pretty(&record).map_err(DaemonError::JsonError)?;

        std::fs::write(&metadata_path, metadata_json)
            .map_err(|e| DaemonError::StorageError(format!("Metadata write: {}", e)))?;

        // Save the embedding
        let embedding_path = user_dir.join(format!("{}.embedding.json", record.face_id));
        let embedding_json =
            serde_json::to_string_pretty(&embedding).map_err(DaemonError::JsonError)?;

        std::fs::write(&embedding_path, embedding_json)
            .map_err(|e| DaemonError::StorageError(format!("Embedding write: {}", e)))?;

        debug!(
            "Face saved: user_id={}, face_id={}",
            record.user_id, record.face_id
        );

        Ok(())
    }

    /// Load an embedding by face_id
    pub fn load_face_embedding(
        &self,
        user_id: u32,
        face_id: &str,
    ) -> Result<Embedding, DaemonError> {
        let user_dir = self.user_dir(user_id)?;
        let embedding_path = user_dir.join(format!("{}.embedding.json", face_id));

        let content = std::fs::read_to_string(&embedding_path)
            .map_err(|e| DaemonError::StorageError(format!("Embedding read: {}", e)))?;

        let embedding: hello_face_core::Embedding =
            serde_json::from_str(&content).map_err(DaemonError::JsonError)?;

        Ok(embedding)
    }

    /// List all faces of a user
    pub fn list_user_faces(&self, user_id: u32) -> Result<Vec<FaceRecord>, DaemonError> {
        let user_dir = self.user_dir(user_id)?;

        if !user_dir.exists() {
            return Ok(Vec::new());
        }

        let mut faces = Vec::new();

        for entry in std::fs::read_dir(&user_dir)
            .map_err(|e| DaemonError::StorageError(format!("User dir read: {}", e)))?
        {
            let entry =
                entry.map_err(|e| DaemonError::StorageError(format!("Dir entry: {}", e)))?;
            let path = entry.path();

            // Look for .meta.json files
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".meta.json"))
                .unwrap_or(false)
            {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| DaemonError::StorageError(format!("Meta read: {}", e)))?;

                let record: FaceRecord =
                    serde_json::from_str(&content).map_err(DaemonError::JsonError)?;

                faces.push(record);
            }
        }

        Ok(faces)
    }

    /// Delete a face
    pub fn delete_face(&self, user_id: u32, face_id: &str) -> Result<(), DaemonError> {
        // face_id is attacker-controlled (it comes straight from the D-Bus
        // DeleteFace request body) and, unlike user_id (a plain u32),
        // wasn't validated before being joined onto a path — user_dir()
        // guards against user_id escaping base_path, but a face_id like
        // "../../../../etc/cron.d/x" would still escape *this* user's own
        // directory once appended below. Real face_ids are always
        // "face_<uid>_<timestamp>" (see register_face), so anything outside
        // that safe character set can only be a traversal attempt.
        if !is_safe_face_id(face_id) {
            return Err(DaemonError::AccessDenied(format!(
                "Invalid face_id: {}",
                face_id
            )));
        }

        let user_dir = self.user_dir(user_id)?;

        let meta_path = user_dir.join(format!("{}.meta.json", face_id));
        let emb_path = user_dir.join(format!("{}.embedding.json", face_id));

        if meta_path.exists() {
            std::fs::remove_file(&meta_path)
                .map_err(|e| DaemonError::StorageError(format!("Meta deletion: {}", e)))?;
        }

        if emb_path.exists() {
            std::fs::remove_file(&emb_path)
                .map_err(|e| DaemonError::StorageError(format!("Embedding deletion: {}", e)))?;
        }

        debug!("Face deleted: user_id={}, face_id={}", user_id, face_id);

        Ok(())
    }

    /// Delete all faces of a user
    pub fn delete_all_faces(&self, user_id: u32) -> Result<(), DaemonError> {
        let user_dir = self.user_dir(user_id)?;

        if user_dir.exists() {
            std::fs::remove_dir_all(&user_dir)
                .map_err(|e| DaemonError::StorageError(format!("User dir deletion: {}", e)))?;
        }

        debug!("All faces deleted for user_id={}", user_id);

        Ok(())
    }

    /// Get the user's directory
    fn user_dir(&self, user_id: u32) -> Result<PathBuf, DaemonError> {
        let user_dir = self.base_path.join(format!("users/{}", user_id));

        // Verify we don't escape base_path (security)
        // Use a simpler approach: check that the normalized path starts with base_path
        let normalized_user = user_dir.canonicalize().ok();
        let normalized_base = self.base_path.canonicalize().ok();

        match (normalized_user, normalized_base) {
            (Some(user), Some(base)) => {
                if !user.starts_with(&base) {
                    return Err(DaemonError::AccessDenied(format!(
                        "Path traversal attempt for user_id={}",
                        user_id
                    )));
                }
            }
            _ => {
                // If the path doesn't exist yet, do a simple check
                // Verify that "../" is not in the path
                let user_str = user_dir.to_string_lossy();
                if user_str.contains("..") {
                    return Err(DaemonError::AccessDenied(format!(
                        "Path traversal attempt for user_id={}",
                        user_id
                    )));
                }
            }
        }

        Ok(user_dir)
    }
}

/// Whether `face_id` is safe to join onto a filesystem path. Real face_ids
/// are always `face_<uid>_<timestamp>` (see `register_face`), so this is
/// deliberately narrow — alphanumeric, `_`, and `-` only, non-empty — rather
/// than trying to blocklist `/`/`..`/etc, which is easy to get wrong.
fn is_safe_face_id(face_id: &str) -> bool {
    !face_id.is_empty()
        && face_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_init() {
        let temp = TempDir::new().unwrap();
        let _ = FaceStorage::new(temp.path()).unwrap();

        assert!(temp.path().join("embeddings").exists());
    }

    #[test]
    fn test_save_and_load_face() {
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();

        let record = FaceRecord {
            face_id: "test_face_1".to_string(),
            user_id: 1000,
            embedding_json: "{}".to_string(),
            quality_score: 0.95,
            registered_at: 0,
            context: "test".to_string(),
        };

        let embedding = Embedding {
            vector: vec![0.1, 0.2, 0.3],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        };

        storage.save_face(&record, &embedding).unwrap();

        let loaded = storage.load_face_embedding(1000, "test_face_1").unwrap();
        assert_eq!(loaded.vector.len(), 3);
    }

    #[test]
    fn test_list_faces() {
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();

        let record1 = FaceRecord {
            face_id: "face_1".to_string(),
            user_id: 1000,
            embedding_json: "{}".to_string(),
            quality_score: 0.95,
            registered_at: 0,
            context: "test".to_string(),
        };

        let record2 = FaceRecord {
            face_id: "face_2".to_string(),
            user_id: 1000,
            embedding_json: "{}".to_string(),
            quality_score: 0.92,
            registered_at: 0,
            context: "test".to_string(),
        };

        let embedding = Embedding {
            vector: vec![0.1, 0.2, 0.3],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        };

        storage.save_face(&record1, &embedding).unwrap();
        storage.save_face(&record2, &embedding).unwrap();

        let faces = storage.list_user_faces(1000).unwrap();
        assert_eq!(faces.len(), 2);
    }

    #[test]
    fn test_open_read_only_missing_dir_returns_none_and_creates_nothing() {
        // This is the property the SDDM system listener depends on: checking
        // an arbitrary, not-yet-authenticated user's storage must never
        // create directories in their home as a side effect.
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join("never-enrolled-user");

        let result = FaceStorage::open_read_only(&missing).unwrap();

        assert!(result.is_none());
        assert!(
            !missing.exists(),
            "open_read_only must not create the directory"
        );
    }

    #[test]
    fn test_open_read_only_existing_dir_can_list_faces() {
        let temp = TempDir::new().unwrap();
        // Set up with the side-effecting constructor once, as enrollment
        // would have already done.
        let storage = FaceStorage::new(temp.path()).unwrap();
        let record = FaceRecord {
            face_id: "face_1".to_string(),
            user_id: 1000,
            embedding_json: "[]".to_string(),
            quality_score: 0.9,
            registered_at: 0,
            context: "sddm".to_string(),
        };
        let embedding = Embedding {
            vector: vec![0.1, 0.2],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.9,
            },
        };
        storage.save_face(&record, &embedding).unwrap();
        drop(storage);

        // Now re-open read-only, as the system listener would per-request.
        let reopened = FaceStorage::open_read_only(temp.path())
            .unwrap()
            .expect("directory exists, should return Some");
        let faces = reopened.list_user_faces(1000).unwrap();
        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].face_id, "face_1");
    }

    #[test]
    fn test_is_safe_face_id() {
        assert!(is_safe_face_id("face_1000_1735036800"));
        assert!(is_safe_face_id("face_1"));
        assert!(!is_safe_face_id(""));
        assert!(!is_safe_face_id("../../../etc/cron.d/x"));
        assert!(!is_safe_face_id("../x"));
        assert!(!is_safe_face_id("a/b"));
        assert!(!is_safe_face_id("a.b"));
        assert!(!is_safe_face_id("/etc/passwd"));
    }

    #[test]
    fn test_delete_face_still_works_for_a_legitimate_face_id() {
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();
        let record = FaceRecord {
            face_id: "face_1000_1735036800".to_string(),
            user_id: 1000,
            embedding_json: "{}".to_string(),
            quality_score: 0.95,
            registered_at: 0,
            context: "test".to_string(),
        };
        let embedding = Embedding {
            vector: vec![0.1, 0.2, 0.3],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        };
        storage.save_face(&record, &embedding).unwrap();

        storage
            .delete_face(1000, "face_1000_1735036800")
            .expect("a real, well-formed face_id must still delete successfully");

        assert!(storage
            .load_face_embedding(1000, "face_1000_1735036800")
            .is_err());
    }

    #[test]
    fn test_delete_face_rejects_a_path_traversal_face_id() {
        // Regression test for the path-traversal fix: a crafted face_id
        // must not be able to reach a file outside this user's own
        // directory, even though the file it targets genuinely exists and
        // is genuinely reachable via that many "../" segments.
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();

        // A file outside any user's directory that a traversal could target.
        let canary = temp.path().join("canary.meta.json");
        std::fs::write(&canary, "should not be deleted").unwrap();

        // "users/1000/" is 2 levels deep under temp.path(), so "../../.."
        // reaches temp.path() itself, landing on "canary" once the code
        // appends ".meta.json".
        let result = storage.delete_face(1000, "../../../canary");

        assert!(result.is_err(), "traversal face_id must be rejected");
        assert!(
            canary.exists(),
            "the file outside the user dir must survive"
        );
    }
}
