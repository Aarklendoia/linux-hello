//! Moteur de reconnaissance faciale - Abstraction et types principaux
//!
//! Cette crate expose une API générique et backend-agnostique pour :
//! - Détection de visages dans des frames
//! - Extraction d'embeddings (signatures faciales)
//! - Comparaison d'embeddings et scoring de similarité

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Résultat de détection de visage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceRegion {
    /// Boîte englobante: (x, y, largeur, hauteur) en pixels
    pub bounding_box: (u32, u32, u32, u32),

    /// Confiance en cette détection (0.0 à 1.0)
    pub confidence: f32,

    /// Landmarks optionnels: position des yeux, nez, bouche, etc.
    /// Format: [(x, y), ...] en pixels relatifs à la région
    pub landmarks: Vec<(f32, f32)>,
}

/// Embedding (empreinte faciale) - vecteur haute dimension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// Vecteur de valeurs flottantes (généralement 128, 256 ou 512 dims)
    pub vector: Vec<f32>,

    /// Métadonnées d'extraction
    pub metadata: EmbeddingMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingMetadata {
    /// Model utilisé pour extraire (ex: "arcface_mobilenet", "facenet")
    pub model: String,

    /// Version du modèle
    pub model_version: String,

    /// Timestamp d'extraction (Unix timestamp)
    pub extracted_at: u64,

    /// Qualité estimée de l'extraction (0.0-1.0)
    pub quality_score: f32,
}

/// Résultat de vérification faciale
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchResult {
    /// Correspondance réussie
    Success {
        /// Score de similarité (0.0 à 1.0 ou distance euclidienne)
        score: f32,
        /// Méthode utilisée
        method: String,
    },

    /// Aucun visage détecté
    NoFace,

    /// Visage détecté mais confiance insuffisante
    LowConfidence { score: f32, required_threshold: f32 },

    /// Utilisateur a annulé
    AbortedByUser,

    /// Erreur interne
    InternalError(String),
}

impl fmt::Display for MatchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchResult::Success { score, method } => {
                write!(f, "Succès ({}): score {:.2}", method, score)
            }
            MatchResult::NoFace => write!(f, "Aucun visage détecté"),
            MatchResult::LowConfidence {
                score,
                required_threshold,
            } => {
                write!(
                    f,
                    "Confiance insuffisante: {:.2} < {:.2}",
                    score, required_threshold
                )
            }
            MatchResult::AbortedByUser => write!(f, "Annulé par l'utilisateur"),
            MatchResult::InternalError(e) => write!(f, "Erreur interne: {}", e),
        }
    }
}

/// Erreurs possibles du moteur de reconnaissance
#[derive(Debug, Error)]
pub enum FaceError {
    #[error("Détection échouée: {0}")]
    DetectionFailed(String),

    #[error("Extraction d'embedding échouée: {0}")]
    EmbeddingFailed(String),

    #[error("Frame invalide: {0}")]
    InvalidFrame(String),

    #[error("Aucun backend disponible")]
    NoBackendAvailable,

    #[error("Erreur de configuration: {0}")]
    ConfigError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Trait pour implémentation de détection de visages
pub trait FaceDetector: Send + Sync {
    /// Détecter les visages dans une frame RGB/grayscale
    ///
    /// # Arguments
    /// * `frame_data` - données brutes des pixels
    /// * `width` - largeur en pixels
    /// * `height` - hauteur en pixels
    /// * `channels` - nombre de canaux (1 pour grayscale, 3 pour RGB)
    ///
    /// # Returns
    /// Vecteur de régions de visages détectés
    fn detect(
        &self,
        frame_data: &[u8],
        width: u32,
        height: u32,
        channels: u32,
    ) -> Result<Vec<FaceRegion>, FaceError>;

    /// Nom du détecteur (ex: "retinaface", "yolov8-face")
    fn name(&self) -> &str;

    /// Version du modèle utilisé
    fn model_version(&self) -> &str;
}

/// Trait pour implémentation d'extraction d'embeddings
pub trait EmbeddingExtractor: Send + Sync {
    /// Extraire un embedding d'une région faciale
    ///
    /// # Arguments
    /// * `face_region` - région détectée du visage
    /// * `frame_data` - données brutes de la frame
    /// * `width` - largeur de la frame
    /// * `height` - hauteur de la frame
    /// * `channels` - canaux
    ///
    /// # Returns
    /// Embedding haute dimension
    fn extract(
        &self,
        face_region: &FaceRegion,
        frame_data: &[u8],
        width: u32,
        height: u32,
        channels: u32,
    ) -> Result<Embedding, FaceError>;

    /// Nom du modèle (ex: "arcface", "facenet")
    fn model_name(&self) -> &str;

    /// Version
    fn model_version(&self) -> &str;

    /// Dimension attendue du vecteur
    fn embedding_dimension(&self) -> usize;
}

/// Trait pour calcul de similarité/distance
pub trait SimilarityMetric: Send + Sync {
    /// Comparer deux embeddings
    ///
    /// # Returns
    /// Score entre 0.0 et 1.0 (ou distance selon la métrique)
    /// Plus élevé = plus similaire
    fn compare(&self, embedding1: &Embedding, embedding2: &Embedding) -> Result<f32, FaceError>;

    /// Nom de la métrique (ex: "euclidean", "cosine")
    fn metric_name(&self) -> &str;
}

/// Configuration de vérification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Seuil de confiance requis (0.0 à 1.0)
    pub confidence_threshold: f32,

    /// Seuil de similarité requis
    pub similarity_threshold: f32,

    /// Timeout max pour la détection (ms)
    pub detection_timeout_ms: u64,

    /// Nombre de tentatives autorisées
    pub max_attempts: u32,

    /// Contexte (login, sudo, screenlock, sddm)
    pub context: String,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.95,
            similarity_threshold: 0.6,
            detection_timeout_ms: 5000,
            max_attempts: 3,
            context: "default".to_string(),
        }
    }
}

/// Implémentation simple par histogramme pour prototype
pub mod simple_implementation {
    use super::*;
    use std::time::SystemTime;

    /// Détecteur simple par analyse de contrastes locaux
    pub struct SimpleDetector;

    impl FaceDetector for SimpleDetector {
        fn detect(
            &self,
            frame_data: &[u8],
            width: u32,
            height: u32,
            _channels: u32,
        ) -> Result<Vec<FaceRegion>, FaceError> {
            // Heuristique simple: chercher les régions de haute variance (caractéristiques faciales)
            // Pour prototype, on retourne une région centrale fixe (où les visages sont généralement)
            // Une vraie implémentation utiliserait Haar Cascade ou YOLO

            let region_w = (width as f32 * 0.5) as u32;
            let region_h = (height as f32 * 0.6) as u32;
            let region_x = (width as f32 * 0.25) as u32;
            let region_y = (height as f32 * 0.2) as u32;

            Ok(vec![FaceRegion {
                bounding_box: (region_x, region_y, region_w, region_h),
                confidence: 0.8,
                landmarks: vec![],
            }])
        }

        fn name(&self) -> &str {
            "SimpleDetector"
        }

        fn model_version(&self) -> &str {
            "0.1"
        }
    }

    /// Extracteur d'embedding simple par histogramme
    pub struct SimpleEmbedder;

    impl EmbeddingExtractor for SimpleEmbedder {
        fn extract(
            &self,
            face_region: &FaceRegion,
            frame_data: &[u8],
            width: u32,
            _height: u32,
            channels: u32,
        ) -> Result<Embedding, FaceError> {
            let (x, y, w, h) = face_region.bounding_box;

            // Extraire l'histogramme de la région du visage
            let mut hist_r = [0u32; 32];
            let mut hist_g = [0u32; 32];
            let mut hist_b = [0u32; 32];

            for py in y..std::cmp::min(y + h, _height) {
                for px in x..std::cmp::min(x + w, width) {
                    let idx = ((py * width + px) * channels) as usize;
                    if idx + 2 < frame_data.len() {
                        let r = frame_data[idx];
                        let g = frame_data[idx + 1];
                        let b = frame_data[idx + 2];

                        hist_r[(r as usize) >> 3] += 1;
                        hist_g[(g as usize) >> 3] += 1;
                        hist_b[(b as usize) >> 3] += 1;
                    }
                }
            }

            // Normaliser l'histogramme en vecteur
            let total = (w * h) as f32;
            let mut vector = Vec::with_capacity(96);

            for h_val in hist_r.iter() {
                vector.push(*h_val as f32 / total);
            }
            for h_val in hist_g.iter() {
                vector.push(*h_val as f32 / total);
            }
            for h_val in hist_b.iter() {
                vector.push(*h_val as f32 / total);
            }

            Ok(Embedding {
                vector,
                metadata: EmbeddingMetadata {
                    model: "histogram".to_string(),
                    model_version: "0.1".to_string(),
                    extracted_at: SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    quality_score: 0.7,
                },
            })
        }

        fn model_name(&self) -> &str {
            "SimpleHistogram"
        }

        fn model_version(&self) -> &str {
            "0.1"
        }

        fn embedding_dimension(&self) -> usize {
            96 // 32 bins par canal RGB
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_serialization() {
        let emb = Embedding {
            vector: vec![0.1, 0.2, 0.3],
            metadata: EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        };

        let json = serde_json::to_string(&emb).unwrap();
        let restored: Embedding = serde_json::from_str(&json).unwrap();
        assert_eq!(emb.vector, restored.vector);
    }

    #[test]
    fn test_match_result_display() {
        let result = MatchResult::Success {
            score: 0.85,
            method: "cosine".to_string(),
        };
        assert!(result.to_string().contains("0.85"));
    }
}
