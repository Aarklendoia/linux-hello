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

impl Default for FaceMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FaceMatcher {
    /// Create a new matcher. Thresholds are fixed (see `context_thresholds`
    /// below) — this crate previously accepted a `default_threshold`
    /// parameter here, but every caller's value was silently discarded; see
    /// the removed `--similarity-threshold` CLI flag/`DaemonConfig` field.
    pub fn new() -> Self {
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

    /// Matching with an independent liveness check filter — IR when
    /// available, otherwise a weaker RGB-only fallback.
    ///
    /// Two-stage architecture:
    ///   1. Liveness check: score >= gate → real face confirmed
    ///   2. Recognition: rgb_score >= threshold → correct person confirmed
    ///
    /// If the liveness score is below its gate, an anti-spoofing failure is
    /// returned. `rgb_liveness` is only consulted when `ir_liveness` is
    /// `None` — with an IR camera, its (well-validated) gate is used alone,
    /// unchanged from before.
    ///
    /// This separation prevents the quality of the liveness signal from
    /// penalizing the recognition score, and vice versa.
    pub fn match_with_liveness(
        &self,
        probe: &Embedding,
        stored: &HashMap<String, Embedding>,
        context: &str,
        ir_liveness: Option<f32>,
        rgb_liveness: f32,
    ) -> MatchResult {
        // Liveness check thresholds, independent of the recognition
        // threshold.
        //
        // LIVENESS_GATE (IR): 0.20 is calibrated to accept low-signal IR
        // cameras while blocking photos (near-zero texture → score < 0.10).
        //
        // RGB_LIVENESS_GATE: see `hello_face_core::liveness::rgb_liveness_score`
        // for the real-hardware calibration this is based on (one subject,
        // one phone, three sessions) — live scored 1.000 every time, a
        // phone-screen replay scored 0.0-0.453. 0.55 sits with a wide
        // margin below every live sample observed and above every spoof
        // sample observed; still a much weaker guarantee than the IR gate,
        // since it rests on far less validation.
        const LIVENESS_GATE: f32 = 0.20;
        const RGB_LIVENESS_GATE: f32 = 0.55;

        // First compute the best RGB score
        let rgb_result = self.match_embedding(probe, stored, context);

        let (liveness, gate, source) = match ir_liveness {
            Some(ir) => (ir.clamp(0.0, 1.0), LIVENESS_GATE, "IR"),
            None => (
                rgb_liveness.clamp(0.0, 1.0),
                RGB_LIVENESS_GATE,
                "RGB-fallback",
            ),
        };

        info!(
            "Liveness gate ({source}): score={:.3}, gate={:.2}, rgb_best={:.3}, rgb_matched={}",
            liveness, gate, rgb_result.best_score, rgb_result.matched
        );

        if liveness < gate {
            // Liveness signal too weak to be a real face (photo, spoofing)
            info!(
                "Liveness gate ({source}): REJECTED (score {:.3} < {:.2})",
                liveness, gate
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
        let matcher = FaceMatcher::new();

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
        let matcher = FaceMatcher::new();

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
        let matcher = FaceMatcher::new();

        assert_eq!(matcher.get_threshold("login"), 0.60);
        assert_eq!(matcher.get_threshold("sudo"), 0.62);
        assert_eq!(matcher.get_threshold("unknown"), 0.58); // default
    }

    fn matching_probe_and_stored() -> (Embedding, HashMap<String, Embedding>) {
        let probe = Embedding {
            vector: vec![1.0, 0.0, 0.0],
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
                vector: vec![1.0, 0.0, 0.0],
                metadata: hello_face_core::EmbeddingMetadata {
                    model: "test".to_string(),
                    model_version: "0.1.0".to_string(),
                    extracted_at: 0,
                    quality_score: 0.9,
                },
            },
        );
        (probe, stored)
    }

    #[test]
    fn test_match_with_liveness_ir_gate_rejects_below_threshold() {
        let matcher = FaceMatcher::new();
        let (probe, stored) = matching_probe_and_stored();

        // A recognizable face, but IR liveness reads as a flat photo
        // (well below the 0.20 IR gate) — must be rejected despite the
        // strong RGB match.
        let result = matcher.match_with_liveness(&probe, &stored, "test", Some(0.05), 1.0);

        assert!(!result.matched);
        assert_eq!(result.face_id, None);
    }

    #[test]
    fn test_match_with_liveness_ir_gate_accepts_above_threshold() {
        let matcher = FaceMatcher::new();
        let (probe, stored) = matching_probe_and_stored();

        let result = matcher.match_with_liveness(&probe, &stored, "test", Some(0.9), 1.0);

        assert!(result.matched);
        assert_eq!(result.face_id, Some("face_1".to_string()));
    }

    #[test]
    fn test_match_with_liveness_falls_back_to_rgb_gate_without_ir() {
        let matcher = FaceMatcher::new();
        let (probe, stored) = matching_probe_and_stored();

        // No IR camera (None): a strong RGB match must still be rejected
        // if the RGB-only liveness fallback reads as a likely screen
        // replay (below RGB_LIVENESS_GATE) — this is the path that used to
        // skip liveness entirely.
        let rejected = matcher.match_with_liveness(&probe, &stored, "test", None, 0.2);
        assert!(!rejected.matched, "low RGB liveness must reject the match");

        // And accept when the RGB fallback reads as a real face.
        let accepted = matcher.match_with_liveness(&probe, &stored, "test", None, 0.95);
        assert!(
            accepted.matched,
            "high RGB liveness must let the match through"
        );
        assert_eq!(accepted.face_id, Some("face_1".to_string()));
    }
}
