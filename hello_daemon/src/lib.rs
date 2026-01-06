//! Implémentation du daemon de reconnaissance faciale
//!
//! Gère:
//! - Stockage des embeddings faciales
//! - Interface D-Bus pour enregistrement/vérification
//! - Accès caméra

use std::path::PathBuf;
use thiserror::Error;

pub mod dbus_interface;

use dbus_interface::{DeleteFaceRequest, RegisterFaceRequest, VerifyRequest, VerifyResult};

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
            let home = std::env::var("HOME")
                .unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home)
                .join(".local/share/linux-hello")
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

/// Impl du service D-Bus
pub struct FaceAuthDaemon {
    config: DaemonConfig,
    // À ajouter: storage manager, camera manager, etc.
}

impl FaceAuthDaemon {
    pub fn new(config: DaemonConfig) -> Result<Self, DaemonError> {
        // Créer les répertoires nécessaires
        std::fs::create_dir_all(&config.storage_path)
            .map_err(|e| DaemonError::StorageError(e.to_string()))?;

        Ok(Self { config })
    }

    pub async fn register_face(
        &self,
        request: RegisterFaceRequest,
    ) -> Result<String, DaemonError> {
        // Vérifier permissions: seul l'utilisateur ou root peut enregistrer son propre visage
        self.check_user_permission(request.user_id)?;

        // TODO: Implémenter capture caméra + extraction embedding
        Err(DaemonError::StorageError(
            "Non implémenté".to_string(),
        ))
    }

    pub async fn delete_face(&self, request: DeleteFaceRequest) -> Result<(), DaemonError> {
        self.check_user_permission(request.user_id)?;

        // TODO: Implémenter suppression
        Err(DaemonError::StorageError(
            "Non implémenté".to_string(),
        ))
    }

    pub async fn verify(&self, request: VerifyRequest) -> Result<VerifyResult, DaemonError> {
        // Vérifier permissions: tout utilisateur peut vérifier son propre visage
        self.check_user_permission(request.user_id)?;

        // TODO: Implémenter vérification
        Ok(VerifyResult::Error {
            message: "Non implémenté".to_string(),
        })
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
            "UID {} peut pas accéder à UID {}",
            current_uid, target_uid
        )))
    }

    pub fn config(&self) -> &DaemonConfig {
        &self.config
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
