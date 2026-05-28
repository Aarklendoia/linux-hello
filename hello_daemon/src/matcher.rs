//! Logique de comparaison et matching des visages
//!
//! Calcule les similarités, applique les seuils, etc.

use hello_face_core::Embedding;
use std::collections::HashMap;
use tracing::{debug, info};

/// Résultat d'une comparaison
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// ID du visage le plus similaire
    pub face_id: Option<String>,

    /// Score de similarité (0.0 à 1.0)
    pub best_score: f32,

    /// Seuil utilisé
    pub threshold: f32,

    /// Tous les scores (face_id -> score)
    pub all_scores: HashMap<String, f32>,

    /// Match réussi ?
    pub matched: bool,
}

/// Gestionnaire de matching de visages
pub struct FaceMatcher {
    /// Seuil de similarité par défaut
    default_threshold: f32,

    /// Seuils par contexte
    context_thresholds: HashMap<String, f32>,
}

impl FaceMatcher {
    /// Créer un nouveau matcher
    pub fn new(default_threshold: f32) -> Self {
        let mut context_thresholds = HashMap::new();

        // Thresholds plus stricts pour les contextes sensibles
        context_thresholds.insert("login".to_string(), 0.65);
        context_thresholds.insert("sudo".to_string(), 0.70);
        context_thresholds.insert("sddm".to_string(), 0.65);
        context_thresholds.insert("screenlock".to_string(), 0.60);
        context_thresholds.insert("test".to_string(), 0.50);

        Self {
            default_threshold,
            context_thresholds,
        }
    }

    /// Obtenir le seuil pour un contexte
    pub fn get_threshold(&self, context: &str) -> f32 {
        self.context_thresholds
            .get(context)
            .copied()
            .unwrap_or(self.default_threshold)
    }

    /// Comparer une probe embedding avec plusieurs embeddings stockés
    ///
    /// # Arguments
    /// * `probe` - L'embedding à vérifier
    /// * `stored` - HashMap de face_id -> embedding
    /// * `context` - Le contexte d'authentification
    ///
    /// # Returns
    /// MatchResult avec les scores et décision
    pub fn match_embedding(
        &self,
        probe: &Embedding,
        stored: &HashMap<String, Embedding>,
        context: &str,
    ) -> MatchResult {
        let threshold = self.get_threshold(context);

        info!(
            "Matching probe vs {} stored faces, context={}, threshold={:.2}",
            stored.len(),
            context,
            threshold
        );

        let mut best_score = 0.0;
        let mut best_face_id = None;
        let mut all_scores = HashMap::new();

        for (face_id, stored_emb) in stored {
            let score = self.cosine_similarity(&probe.vector, &stored_emb.vector);
            all_scores.insert(face_id.clone(), score);

            debug!("Face {} score: {:.4}", face_id, score);

            if score > best_score {
                best_score = score;
                best_face_id = Some(face_id.clone());
            }
        }

        let matched = best_score >= threshold;

        info!(
            "Best match: {} (score={:.4}, matched={})",
            best_face_id.as_deref().unwrap_or("none"),
            best_score,
            matched
        );

        MatchResult {
            face_id: if matched { best_face_id } else { None },
            best_score,
            threshold,
            all_scores,
            matched,
        }
    }

    /// Matching avec fusion RGB + score de vivacité IR
    ///
    /// `ir_liveness` : score IR dans [0, 1] calculé par `ir_liveness_score()`,
    ///                 ou `None` si la caméra IR est absente/indisponible.
    ///
    /// Score final = 0.7 × score_rgb + 0.3 × ir_liveness  (si IR présent)
    ///             = score_rgb                             (si pas d'IR)
    pub fn match_with_liveness(
        &self,
        probe: &Embedding,
        stored: &HashMap<String, Embedding>,
        context: &str,
        ir_liveness: Option<f32>,
    ) -> MatchResult {
        // D'abord calculer le meilleur score RGB
        let rgb_result = self.match_embedding(probe, stored, context);

        // Si pas d'IR, retourner le résultat RGB direct
        let Some(liveness) = ir_liveness else {
            return rgb_result;
        };

        let liveness = liveness.clamp(0.0, 1.0);
        let threshold = self.get_threshold(context);

        // Recalculer tous les scores avec la fusion
        let fused_scores: HashMap<String, f32> = rgb_result
            .all_scores
            .iter()
            .map(|(id, &rgb_score)| {
                let fused = 0.7 * rgb_score + 0.3 * liveness;
                (id.clone(), fused)
            })
            .collect();

        let (best_face_id, best_score) = fused_scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(id, &s)| (Some(id.clone()), s))
            .unwrap_or((None, 0.0));

        let matched = best_score >= threshold;

        info!(
            "Liveness fusion: rgb_best={:.3}, ir_liveness={:.3}, fused={:.3}, matched={}",
            rgb_result.best_score, liveness, best_score, matched
        );

        MatchResult {
            face_id: if matched { best_face_id } else { None },
            best_score,
            threshold,
            all_scores: fused_scores,
            matched,
        }
    }

    /// Calculer la similarité cosinus entre deux vecteurs
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }

        let min_len = a.len().min(b.len());

        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for i in 0..min_len {
            dot_product += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        norm_a = norm_a.sqrt();
        norm_b = norm_b.sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        (dot_product / (norm_a * norm_b)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let matcher = FaceMatcher::new(0.6);

        // Vecteurs identiques = 1.0
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let sim = matcher.cosine_similarity(&v1, &v2);
        assert!((sim - 1.0).abs() < 0.001);

        // Vecteurs perpendiculaires = 0.0
        let v1 = vec![1.0, 0.0];
        let v2 = vec![0.0, 1.0];
        let sim = matcher.cosine_similarity(&v1, &v2);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_match_embedding() {
        let matcher = FaceMatcher::new(0.6);

        let probe = Embedding {
            vector: vec![1.0, 0.0, 0.0, 0.0, 0.0],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.9,
            },
        };

        let mut stored = HashMap::new();
        stored.insert(
            "face_1".to_string(),
            Embedding {
                vector: vec![1.0, 0.0, 0.0, 0.0, 0.0],
                metadata: hello_face_core::EmbeddingMetadata {
                    model: "test".to_string(),
                    model_version: "0.1.0".to_string(),
                    extracted_at: 0,
                    quality_score: 0.9,
                },
            },
        );
        stored.insert(
            "face_2".to_string(),
            Embedding {
                vector: vec![0.0, 1.0, 0.0, 0.0, 0.0],
                metadata: hello_face_core::EmbeddingMetadata {
                    model: "test".to_string(),
                    model_version: "0.1.0".to_string(),
                    extracted_at: 0,
                    quality_score: 0.9,
                },
            },
        );

        let result = matcher.match_embedding(&probe, &stored, "test");

        assert_eq!(result.face_id, Some("face_1".to_string()));
        assert!(result.matched);
    }

    #[test]
    fn test_context_thresholds() {
        let matcher = FaceMatcher::new(0.6);

        assert_eq!(matcher.get_threshold("login"), 0.65);
        assert_eq!(matcher.get_threshold("sudo"), 0.70);
        assert_eq!(matcher.get_threshold("unknown"), 0.6); // default
    }
}
