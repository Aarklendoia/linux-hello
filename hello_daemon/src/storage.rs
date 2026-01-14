//! Gestion persistante du stockage des embeddings faciales
//!
//! - SQLite pour métadonnées (user_id, face_id, quality_score, registered_at)
//! - Fichiers JSON pour les embeddings (pour flexibilité)

use crate::{DaemonError, FaceRecord};
use hello_face_core::Embedding;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Gestionnaire de stockage des visages
pub struct FaceStorage {
    /// Répertoire racine de stockage
    base_path: PathBuf,

    /// Chemin vers la DB SQLite
    #[allow(dead_code)]
    db_path: PathBuf,
}

impl FaceStorage {
    /// Créer un nouveau gestionnaire de stockage
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self, DaemonError> {
        let base_path = base_path.as_ref().to_path_buf();

        // Créer la structure de répertoires
        std::fs::create_dir_all(&base_path).map_err(|e| {
            DaemonError::StorageError(format!("Création répertoire échouée: {}", e))
        })?;

        let db_path = base_path.join("faces.db");

        let storage = Self { base_path, db_path };

        // Initialiser la DB si elle n'existe pas
        storage.init_db()?;

        Ok(storage)
    }

    /// Initialiser la structure SQLite
    fn init_db(&self) -> Result<(), DaemonError> {
        // Créer le répertoire embeddings
        let embeddings_dir = self.base_path.join("embeddings");
        std::fs::create_dir_all(&embeddings_dir)
            .map_err(|e| DaemonError::StorageError(format!("Création embeddings dir: {}", e)))?;

        // Pour maintenant, on utilise des fichiers JSON
        // La migration vers SQLite se fera plus tard avec sqlx async
        info!("Stockage initialisé à: {}", self.base_path.display());

        Ok(())
    }

    /// Sauvegarder un nouveau visage enregistré
    pub fn save_face(&self, record: &FaceRecord, embedding: &Embedding) -> Result<(), DaemonError> {
        // Vérifier permissions du répertoire user
        let user_dir = self.user_dir(record.user_id)?;
        std::fs::create_dir_all(&user_dir)
            .map_err(|e| DaemonError::StorageError(format!("Création user dir: {}", e)))?;

        // Sauvegarder métadonnées dans un fichier JSON
        let metadata_path = user_dir.join(format!("{}.meta.json", record.face_id));
        let metadata_json =
            serde_json::to_string_pretty(&record).map_err(DaemonError::JsonError)?;

        std::fs::write(&metadata_path, metadata_json)
            .map_err(|e| DaemonError::StorageError(format!("Écriture métadonnées: {}", e)))?;

        // Sauvegarder embedding
        let embedding_path = user_dir.join(format!("{}.embedding.json", record.face_id));
        let embedding_json =
            serde_json::to_string_pretty(&embedding).map_err(DaemonError::JsonError)?;

        std::fs::write(&embedding_path, embedding_json)
            .map_err(|e| DaemonError::StorageError(format!("Écriture embedding: {}", e)))?;

        debug!(
            "Visage sauvegardé: user_id={}, face_id={}",
            record.user_id, record.face_id
        );

        Ok(())
    }

    /// Charger un embedding par face_id
    pub fn load_face_embedding(
        &self,
        user_id: u32,
        face_id: &str,
    ) -> Result<Embedding, DaemonError> {
        let user_dir = self.user_dir(user_id)?;
        let embedding_path = user_dir.join(format!("{}.embedding.json", face_id));

        let content = std::fs::read_to_string(&embedding_path)
            .map_err(|e| DaemonError::StorageError(format!("Lecture embedding: {}", e)))?;

        let embedding: hello_face_core::Embedding =
            serde_json::from_str(&content).map_err(DaemonError::JsonError)?;

        Ok(embedding)
    }

    /// Lister tous les visages d'un utilisateur
    pub fn list_user_faces(&self, user_id: u32) -> Result<Vec<FaceRecord>, DaemonError> {
        let user_dir = self.user_dir(user_id)?;

        if !user_dir.exists() {
            return Ok(Vec::new());
        }

        let mut faces = Vec::new();

        for entry in std::fs::read_dir(&user_dir)
            .map_err(|e| DaemonError::StorageError(format!("Lecture user dir: {}", e)))?
        {
            let entry =
                entry.map_err(|e| DaemonError::StorageError(format!("Entrée dir: {}", e)))?;
            let path = entry.path();

            // Chercher les fichiers .meta.json
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".meta.json"))
                .unwrap_or(false)
            {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| DaemonError::StorageError(format!("Lecture meta: {}", e)))?;

                let record: FaceRecord =
                    serde_json::from_str(&content).map_err(DaemonError::JsonError)?;

                faces.push(record);
            }
        }

        Ok(faces)
    }

    /// Supprimer un visage
    pub fn delete_face(&self, user_id: u32, face_id: &str) -> Result<(), DaemonError> {
        let user_dir = self.user_dir(user_id)?;

        let meta_path = user_dir.join(format!("{}.meta.json", face_id));
        let emb_path = user_dir.join(format!("{}.embedding.json", face_id));

        if meta_path.exists() {
            std::fs::remove_file(&meta_path)
                .map_err(|e| DaemonError::StorageError(format!("Suppression meta: {}", e)))?;
        }

        if emb_path.exists() {
            std::fs::remove_file(&emb_path)
                .map_err(|e| DaemonError::StorageError(format!("Suppression embedding: {}", e)))?;
        }

        debug!("Visage supprimé: user_id={}, face_id={}", user_id, face_id);

        Ok(())
    }

    /// Supprimer tous les visages d'un utilisateur
    pub fn delete_all_faces(&self, user_id: u32) -> Result<(), DaemonError> {
        let user_dir = self.user_dir(user_id)?;

        if user_dir.exists() {
            std::fs::remove_dir_all(&user_dir)
                .map_err(|e| DaemonError::StorageError(format!("Suppression user dir: {}", e)))?;
        }

        debug!("Tous les visages supprimés pour user_id={}", user_id);

        Ok(())
    }

    /// Obtenir le répertoire de l'utilisateur
    fn user_dir(&self, user_id: u32) -> Result<PathBuf, DaemonError> {
        let user_dir = self.base_path.join(format!("users/{}", user_id));

        // Vérifier qu'on ne sort pas du base_path (sécurité)
        // Utiliser une approche plus simple: vérifier que le chemin normalisé commence par base_path
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
                // Si le chemin n'existe pas encore, faire une vérification simple
                // Vérifier que "../" n'est pas dans le chemin
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
