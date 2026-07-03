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
        std::fs::create_dir_all(&base_path).map_err(|e| {
            DaemonError::StorageError(format!("Directory creation failed: {}", e))
        })?;

        let db_path = base_path.join("faces.db");

        let storage = Self { base_path, db_path };

        // Initialize the DB if it doesn't exist
        storage.init_db()?;

        Ok(storage)
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
}
