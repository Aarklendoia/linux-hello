//! Face comparison and matching logic
//!
//! Computes similarities, applies thresholds, etc.

use hello_face_core::Embedding;
use std::collections::HashMap;
use tracing::{debug, info};

/// Result of a comparison
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// ID of the most similar face
    pub face_id: Option<String>,

    /// Similarity score (0.0 to 1.0)
    pub best_score: f32,

    /// Threshold used
    pub threshold: f32,

    /// All scores (face_id -> score)
    pub all_scores: HashMap<String, f32>,

    /// Match succeeded?
    pub matched: bool,
}

/// Face matching manager
pub struct FaceMatcher {
    /// Default similarity threshold
    default_threshold: f32,

    /// Thresholds per context
    context_thresholds: HashMap<String, f32>,
}

impl FaceMatcher {
    /// Create a new matcher
    pub fn new(_default_threshold: f32) -> Self {
        let default_threshold = 0.58;
        let mut context_thresholds = HashMap::new();

        // Stricter thresholds for sensitive contexts
        context_thresholds.insert("login".to_string(), 0.60);
        context_thresholds.insert("sudo".to_string(), 0.62);
        context_thresholds.insert("polkit".to_string(), 0.60);
        context_thresholds.insert("sddm".to_string(), 0.60);
        context_thresholds.insert("screenlock".to_string(), 0.55);
        context_thresholds.insert("test".to_string(), 0.50);

        Self {
            default_threshold,
            context_thresholds,
        }
    }

    /// Get the threshold for a context
    pub fn get_threshold(&self, context: &str) -> f32 {
        self.context_thresholds
            .get(context)
            .copied()
            .unwrap_or(self.default_threshold)
    }

    /// Compare a probe embedding against several stored embeddings
    ///
    /// # Arguments
    /// * `probe` - The embedding to verify
    /// * `stored` - HashMap of face_id -> embedding
    /// * `context` - The authentication context
    ///
    /// # Returns
    /// MatchResult with the scores and decision
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

    /// Matching with an independent IR liveness check filter
    ///
    /// Two-stage architecture:
    ///   1. Liveness check: ir_liveness >= LIVENESS_GATE → real face confirmed
    ///   2. Recognition: rgb_score >= threshold → correct person confirmed
    ///
    /// If ir_liveness < LIVENESS_GATE, an anti-spoofing failure is returned.
    /// If ir_liveness is None (no IR camera), the filter is not applied.
    ///
    /// This separation prevents the quality of the IR signal from penalizing
    /// the recognition score, and vice versa.
    pub fn match_with_liveness(
        &self,
        probe: &Embedding,
        stored: &HashMap<String, Embedding>,
        context: &str,
        ir_liveness: Option<f32>,
    ) -> MatchResult {
        // Liveness check threshold, independent of the recognition threshold.
        // 0.20 is calibrated to accept low-signal IR cameras while
        // blocking photos (near-zero texture → score < 0.10).
        const LIVENESS_GATE: f32 = 0.20;

        // First compute the best RGB score
        let rgb_result = self.match_embedding(probe, stored, context);

        let Some(liveness) = ir_liveness else {
            // No IR camera → liveness cannot be checked, so accept
            return rgb_result;
        };

        let liveness = liveness.clamp(0.0, 1.0);

        info!(
            "Liveness gate: ir_liveness={:.3}, gate={:.2}, rgb_best={:.3}, rgb_matched={}",
            liveness, LIVENESS_GATE, rgb_result.best_score, rgb_result.matched
        );

        if liveness < LIVENESS_GATE {
            // IR signal too weak to be a real face (photo, spoofing)
            info!(
                "Liveness gate: REJECTED (score {:.3} < {:.2})",
                liveness, LIVENESS_GATE
            );
            let threshold = self.get_threshold(context);
            return MatchResult {
                face_id: None,
                best_score: rgb_result.best_score,
                threshold,
                all_scores: rgb_result.all_scores,
                matched: false,
            };
        }

        // Liveness confirmed → the RGB result stands
        rgb_result
    }

    /// Compute the cosine similarity between two vectors
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

        // Identical vectors = 1.0
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let sim = matcher.cosine_similarity(&v1, &v2);
        assert!((sim - 1.0).abs() < 0.001);

        // Perpendicular vectors = 0.0
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

        assert_eq!(matcher.get_threshold("login"), 0.60);
        assert_eq!(matcher.get_threshold("sudo"), 0.62);
        assert_eq!(matcher.get_threshold("unknown"), 0.58); // default
    }
}
