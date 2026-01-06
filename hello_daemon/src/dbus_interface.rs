//! API D-Bus exposée par le daemon
//!
//! Interface: com.linuxhello.FaceAuth
//! Chemin: /com/linuxhello/FaceAuth

use serde::{Deserialize, Serialize};
use std::fmt;
use zbus::dbus_interface;

// ============================================================================
// Types de requête/réponse sérialisables
// ============================================================================

/// Requête d'enregistrement de visage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterFaceRequest {
    /// UID de l'utilisateur propriétaire
    pub user_id: u32,
    
    /// Contexte (login, sudo, screenlock, sddm, test)
    pub context: String,
    
    /// Timeout max en ms pour la capture/enregistrement
    pub timeout_ms: u64,
    
    /// Nombre de frames à capturer et moyenner
    pub num_samples: u32,
}

/// Réponse d'enregistrement réussi
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterFaceResponse {
    /// ID unique du modèle enregistré
    pub face_id: String,
    
    /// Timestamp d'enregistrement
    pub registered_at: u64,
    
    /// Score de qualité moyen des embeddings
    pub quality_score: f32,
}

/// Requête de suppression de visage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFaceRequest {
    /// UID de l'utilisateur
    pub user_id: u32,
    
    /// ID du visage à supprimer (None = tous)
    pub face_id: Option<String>,
}

/// Requête de vérification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    /// UID de l'utilisateur à authentifier
    pub user_id: u32,
    
    /// Contexte
    pub context: String,
    
    /// Timeout max en ms
    pub timeout_ms: u64,
}

/// Résultat de vérification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerifyResult {
    /// Succès
    Success {
        face_id: String,
        similarity_score: f32,
    },
    
    /// Aucun visage détecté
    NoFaceDetected,
    
    /// Visage détecté mais non reconnu
    NoMatch { best_score: f32, threshold: f32 },
    
    /// Pas de modèles d'enregistrement
    NoEnrollment,
    
    /// Utilisateur a annulé
    Cancelled,
    
    /// Erreur
    Error { message: String },
}

impl fmt::Display for VerifyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyResult::Success {
                face_id,
                similarity_score,
            } => {
                write!(f, "Succès ({}): {:.2}", face_id, similarity_score)
            }
            VerifyResult::NoFaceDetected => write!(f, "Aucun visage"),
            VerifyResult::NoMatch {
                best_score,
                threshold,
            } => {
                write!(
                    f,
                    "Non reconnu: {:.2} < {:.2}",
                    best_score, threshold
                )
            }
            VerifyResult::NoEnrollment => write!(f, "Pas d'enregistrement"),
            VerifyResult::Cancelled => write!(f, "Annulé"),
            VerifyResult::Error { message } => write!(f, "Erreur: {}", message),
        }
    }
}

// ============================================================================
// Interface D-Bus
// ============================================================================

/// Interface D-Bus pour authentication faciale
pub struct FaceAuthService {
    // À remplir avec impl en lib.rs
}

#[dbus_interface(name = "com.linuxhello.FaceAuth")]
impl FaceAuthService {
    /// Enregistrer un nouveau visage pour un utilisateur
    ///
    /// # Arguments
    /// * `request` - JSON de RegisterFaceRequest
    ///
    /// # Returns
    /// JSON de RegisterFaceResponse ou erreur
    pub async fn register_face(&self, request: &str) -> zbus::fdo::Result<String> {
        // Impl en lib.rs
        Err(zbus::fdo::Error::Failed(
            "Non implémenté".to_string(),
        ))
    }

    /// Supprimer un ou tous les visages
    ///
    /// # Arguments
    /// * `request` - JSON de DeleteFaceRequest
    pub async fn delete_face(&self, request: &str) -> zbus::fdo::Result<()> {
        // Impl en lib.rs
        Err(zbus::fdo::Error::Failed(
            "Non implémenté".to_string(),
        ))
    }

    /// Vérifier l'identité d'un utilisateur
    ///
    /// # Arguments
    /// * `request` - JSON de VerifyRequest
    ///
    /// # Returns
    /// JSON de VerifyResult
    pub async fn verify(&self, request: &str) -> zbus::fdo::Result<String> {
        // Impl en lib.rs
        Err(zbus::fdo::Error::Failed(
            "Non implémenté".to_string(),
        ))
    }

    /// Lister les visages enregistrés pour un utilisateur
    ///
    /// # Arguments
    /// * `user_id` - UID
    ///
    /// # Returns
    /// JSON array de face_ids et metadata
    pub async fn list_faces(&self, user_id: u32) -> zbus::fdo::Result<String> {
        // Impl en lib.rs
        Err(zbus::fdo::Error::Failed(
            "Non implémenté".to_string(),
        ))
    }

    /// Vérifier que le daemon est opérationnel
    pub async fn ping(&self) -> zbus::fdo::Result<String> {
        Ok("pong".to_string())
    }

    /// Version du daemon
    #[dbus_interface(property)]
    pub async fn version(&self) -> zbus::fdo::Result<String> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }

    /// Vérifier si une caméra est disponible
    #[dbus_interface(property)]
    pub async fn camera_available(&self) -> zbus::fdo::Result<bool> {
        // À implémenter avec détection réelle
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
