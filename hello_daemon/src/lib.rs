//! Implémentation du daemon de reconnaissance faciale
//!
//! Gère:
//! - Stockage des embeddings faciales
//! - Interface D-Bus pour enregistrement/vérification
//! - Accès caméra
//! - Matching et scoring

use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

pub mod camera;
pub mod capture_stream;
pub mod dbus;
pub mod dbus_interface;
pub mod matcher;
pub mod pam_helper;
pub mod storage;

use camera::CameraManager;
use dbus_interface::{DeleteFaceRequest, RegisterFaceRequest, VerifyRequest, VerifyResult};
use matcher::FaceMatcher;
use storage::FaceStorage;

/// Erreurs du daemon
#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("Utilisateur non trouvé: {0}")]
    UserNotFound(u32),

    #[error("Visage non trouvé: {0}")]
    FaceNotFound(String),

    #[error("Accès refusé: {0}")]
    AccessDenied(String),

    #[error("Stockage échoué: {0}")]
    StorageError(String),

    #[error("D-Bus error: {0}")]
    DbusError(String),

    #[error("Caméra: {0}")]
    CameraError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Erreur JSON: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Configuration du daemon
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Répertoire de stockage des embeddings
    pub storage_path: PathBuf,

    /// Mode root (true) ou user (false)
    pub root_mode: bool,

    /// UID courant si mode user
    pub current_uid: Option<u32>,

    /// Seuil de similarité par défaut
    pub default_similarity_threshold: f32,

    /// Activer les logs détaillés
    pub debug: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let storage_path = if unsafe { libc::getuid() } == 0 {
            // Mode root: /var/lib/linux-hello/
            PathBuf::from("/var/lib/linux-hello")
        } else {
            // Mode user: ~/.local/share/linux-hello/
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

/// Métadonnées d'un visage enregistré
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FaceRecord {
    /// ID unique
    pub face_id: String,

    /// UID propriétaire
    pub user_id: u32,

    /// Embedding sérialisé (JSON)
    pub embedding_json: String,

    /// Score de qualité
    pub quality_score: f32,

    /// Timestamp d'enregistrement
    pub registered_at: u64,

    /// Contexte d'enregistrement
    pub context: String,
}

/// Impl du service D-Bus avec tous les composants
pub struct FaceAuthDaemon {
    config: DaemonConfig,
    storage: Arc<FaceStorage>,
    camera: Arc<CameraManager>,
    matcher: Arc<FaceMatcher>,
}

impl FaceAuthDaemon {
    pub fn new(config: DaemonConfig) -> Result<Self, DaemonError> {
        // Créer le storage
        let storage = FaceStorage::new(&config.storage_path)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;

        // Créer le camera manager
        let camera = CameraManager::new(5000); // 5s timeout par défaut

        // Créer le matcher avec seuil de config
        let matcher = FaceMatcher::new(config.default_similarity_threshold);

        info!("Daemon créé avec config: {:?}", config);

        Ok(Self {
            config,
            storage: Arc::new(storage),
            camera: Arc::new(camera),
            matcher: Arc::new(matcher),
        })
    }

    pub async fn register_face(&self, request: RegisterFaceRequest) -> Result<String, DaemonError> {
        // Vérifier permissions
        self.check_user_permission(request.user_id)?;

        info!(
            "Enregistrement de visage pour user_id={}, context={}",
            request.user_id, request.context
        );

        // Capturer des frames
        let capture = self
            .camera
            .capture_frames(request.num_samples, request.timeout_ms)
            .await
            .map_err(|e| DaemonError::CameraError(e.to_string()))?;

        // Sélectionner la meilleure embedding (pour MVP: la première)
        let embedding = capture.embeddings.first().ok_or(DaemonError::CameraError(
            "Aucune frame capturée".to_string(),
        ))?;

        // Générer un ID unique pour ce visage
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let face_id = format!("face_{}_{}", request.user_id, now);

        // Créer le record
        let record = FaceRecord {
            face_id: face_id.clone(),
            user_id: request.user_id,
            embedding_json: serde_json::to_string(&embedding.vector)?,
            quality_score: capture.quality_score,
            registered_at: now,
            context: request.context.clone(),
        };

        // Sauvegarder
        self.storage
            .save_face(&record, embedding)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;

        info!("Visage enregistré: face_id={}", face_id);

        // Retourner le JSON de réponse
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
            "Suppression visage pour user_id={}, face_id={:?}",
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
        // Vérifier permissions
        self.check_user_permission(request.user_id)?;

        info!(
            "Vérification pour user_id={}, context={}",
            request.user_id, request.context
        );

        // Charger les visages enregistrés
        let faces = self
            .storage
            .list_user_faces(request.user_id)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;

        if faces.is_empty() {
            info!("Aucun visage enregistré pour user_id={}", request.user_id);
            return Ok(VerifyResult::NoEnrollment);
        }

        // Capturer une frame
        let capture = self
            .camera
            .capture_frames(1, request.timeout_ms)
            .await
            .map_err(|e| DaemonError::CameraError(e.to_string()))?;

        let probe = capture.embeddings.first().ok_or(DaemonError::CameraError(
            "Aucune frame capturée".to_string(),
        ))?;

        // Charger les embeddings stockés
        let mut stored_embeddings = std::collections::HashMap::new();
        for face in &faces {
            let embedding = self
                .storage
                .load_face_embedding(request.user_id, &face.face_id)
                .map_err(|e| DaemonError::StorageError(e.to_string()))?;
            stored_embeddings.insert(face.face_id.clone(), embedding);
        }

        // Matcher
        let match_result =
            self.matcher
                .match_embedding(probe, &stored_embeddings, &request.context);

        if match_result.matched {
            Ok(VerifyResult::Success {
                face_id: match_result.face_id.unwrap(),
                similarity_score: match_result.best_score,
            })
        } else if match_result.best_score > 0.0 {
            Ok(VerifyResult::NoMatch {
                best_score: match_result.best_score,
                threshold: match_result.threshold,
            })
        } else {
            Ok(VerifyResult::NoFaceDetected)
        }
    }

    pub async fn list_faces(&self, user_id: u32) -> Result<String, DaemonError> {
        self.check_user_permission(user_id)?;

        let faces = self
            .storage
            .list_user_faces(user_id)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;

        Ok(serde_json::to_string(&faces)?)
    }

    /// Vérifier que l'utilisateur courant a le droit d'accéder à cet UID
    fn check_user_permission(&self, target_uid: u32) -> Result<(), DaemonError> {
        let current_uid = unsafe { libc::getuid() };

        // Root peut tout faire
        if current_uid == 0 {
            return Ok(());
        }

        // Un utilisateur peut accéder à son propre visage
        if current_uid == target_uid {
            return Ok(());
        }

        Err(DaemonError::AccessDenied(format!(
            "UID {} ne peut pas accéder à UID {}",
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
