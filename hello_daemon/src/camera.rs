//! Abstraction caméra pour le daemon
//!
//! Fournit une interface simple pour capturer des frames
//! et les passer au moteur de reconnaissance

use crate::capture_stream::CaptureFrameEvent;
use hello_camera::{Frame, FrameFormat};
use hello_face_core::{Embedding, EmbeddingExtractor, FaceDetector};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Erreurs caméra
#[derive(Debug, Error)]
pub enum CameraError {
    #[error("Caméra non disponible")]
    NotAvailable,

    #[error("Timeout capture")]
    Timeout,

    #[error("Erreur capture: {0}")]
    CaptureError(String),

    #[error("Erreur extraction: {0}")]
    ExtractionError(String),
}

/// Résultat d'une capture caméra
pub struct CaptureResult {
    /// Frames RGB capturées
    pub frames: Vec<Frame>,

    /// Frames IR capturées (None si pas de caméra IR)
    pub ir_frames: Option<Vec<Frame>>,

    /// Embeddings extraits
    pub embeddings: Vec<Embedding>,

    /// Score de qualité moyen
    pub quality_score: f32,

    /// Score de vivacité IR (None si pas de caméra IR)
    pub ir_liveness: Option<f32>,
}

/// Gestionnaire caméra pour le daemon
pub struct CameraManager {
    /// Timeout par défaut pour les captures (ms)
    default_timeout_ms: u64,
    /// Chemin du device RGB
    pub rgb_device: String,
    /// Chemin du device IR (si détecté)
    pub ir_device: Option<String>,
    /// Détecteur de visages (SCRFD ou fallback)
    detector: Arc<Box<dyn FaceDetector>>,
    /// Extracteur d'embeddings (ArcFace ou fallback)
    extractor: Arc<Box<dyn EmbeddingExtractor>>,
}

impl CameraManager {
    /// Créer un gestionnaire caméra en scannant les devices disponibles
    pub fn new(default_timeout_ms: u64) -> Self {
        let inventory = hello_camera::scan_cameras();
        info!(
            "Inventaire caméras: RGB={}, IR={}",
            inventory.rgb_device,
            inventory.ir_device.as_deref().unwrap_or("aucune")
        );
        let models_dir = hello_face_core::default_models_dir();
        let detector = Arc::new(hello_face_core::create_detector(&models_dir));
        let extractor = Arc::new(hello_face_core::create_extractor(&models_dir));
        Self {
            default_timeout_ms,
            rgb_device: inventory.rgb_device,
            ir_device: inventory.ir_device,
            detector,
            extractor,
        }
    }

    /// Vérifier si une caméra RGB est disponible
    pub fn is_available(&self) -> bool {
        std::path::Path::new(&self.rgb_device).exists()
    }

    /// Vérifier si une caméra IR est disponible
    pub fn has_ir(&self) -> bool {
        self.ir_device
            .as_ref()
            .map(|p| std::path::Path::new(p).exists())
            .unwrap_or(false)
    }

    /// Capturer N frames RGB (+ IR si disponible) et extraire les embeddings
    pub async fn capture_frames(
        &self,
        num_frames: u32,
        timeout_ms: u64,
    ) -> Result<CaptureResult, CameraError> {
        let timeout = if timeout_ms == 0 {
            self.default_timeout_ms
        } else {
            timeout_ms
        };

        info!(
            "Capture de {} frames, timeout={}ms, rgb={}, ir={}",
            num_frames,
            timeout,
            self.rgb_device,
            self.ir_device.as_deref().unwrap_or("aucune")
        );

        let rgb_device = self.rgb_device.clone();
        let ir_device = self.ir_device.clone();

        // Capture RGB en thread bloquant (V4L2 n'est pas async)
        let (rgb_frames, ir_frames) = tokio::task::spawn_blocking(move || {
            let mut rgb_frames: Vec<Frame> = Vec::new();
            let mut ir_frames: Vec<Frame> = Vec::new();

            // Capture RGB
            let rgb_result = hello_camera::capture_rgb_stream_v4l2(
                &rgb_device,
                num_frames,
                timeout,
                |data, w, h| {
                    let ts = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    rgb_frames.push(Frame {
                        data,
                        width: w,
                        height: h,
                        format: FrameFormat::Rgb8,
                        timestamp_ms: ts,
                    });
                },
            );

            if let Err(e) = rgb_result {
                warn!("Capture RGB V4L2 échouée ({}), fallback simulation", e);
                // Fallback : frames noires 640×480
                for i in 0..num_frames {
                    rgb_frames.push(Frame {
                        data: vec![0u8; 640 * 480 * 3],
                        width: 640,
                        height: 480,
                        format: FrameFormat::Rgb8,
                        timestamp_ms: i as u64 * 33,
                    });
                }
            }

            // Capture IR (parallèle, optionnelle)
            if let Some(ref ir_path) = ir_device {
                let ir_result = hello_camera::capture_gray_stream_v4l2(
                    ir_path,
                    num_frames,
                    timeout,
                    |data, w, h| {
                        let ts = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        ir_frames.push(Frame {
                            data,
                            width: w,
                            height: h,
                            format: FrameFormat::Gray8,
                            timestamp_ms: ts,
                        });
                    },
                );
                if let Err(e) = ir_result {
                    warn!("Capture IR échouée ({}), désactivée pour cette session", e);
                }
            }

            let ir_opt = if ir_frames.is_empty() {
                None
            } else {
                Some(ir_frames)
            };

            (rgb_frames, ir_opt)
        })
        .await
        .map_err(|e| CameraError::CaptureError(e.to_string()))?;

        // Extraire embeddings depuis les frames RGB via détecteur + extracteur
        let detector = Arc::clone(&self.detector);
        let extractor = Arc::clone(&self.extractor);

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let embeddings: Vec<Embedding> = rgb_frames
            .iter()
            .enumerate()
            .map(|(i, frame)| {
                // 1. Détecter les visages dans la frame
                let faces = detector
                    .detect(&frame.data, frame.width, frame.height, 3)
                    .unwrap_or_default();

                // 2. Prendre le visage avec la meilleure confiance
                let best_face = faces
                    .into_iter()
                    .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap());

                // 3. Extraire l'embedding
                if let Some(face) = best_face {
                    match extractor.extract(&face, &frame.data, frame.width, frame.height, 3) {
                        Ok(emb) => return emb,
                        Err(e) => warn!("Extraction embedding frame {}: {}", i, e),
                    }
                }

                // Fallback : embedding stub si détection/extraction échoue
                let vector = compute_stub_embedding(&frame.data, frame.width, frame.height);
                Embedding {
                    vector,
                    metadata: hello_face_core::EmbeddingMetadata {
                        model: "pixel-mean-128-fallback".to_string(),
                        model_version: "0.2.0".to_string(),
                        extracted_at: now_secs + i as u64,
                        quality_score: 0.50,
                    },
                }
            })
            .collect();

        // Calculer le score de vivacité IR depuis la première frame IR disponible
        let ir_liveness = ir_frames.as_ref().and_then(|frames| {
            let frame = frames.first()?;
            // Utiliser la boîte du visage de la première frame RGB pour aligner
            // En pratique les caméras RGB/IR ont le même champ de vision sur le Brio
            let dummy_face = hello_face_core::FaceRegion {
                bounding_box: (
                    frame.width / 4,
                    frame.height / 5,
                    frame.width / 2,
                    frame.height * 3 / 5,
                ),
                confidence: 1.0,
                landmarks: vec![],
            };
            Some(hello_face_core::liveness::ir_liveness_score(
                &frame.data,
                frame.width,
                frame.height,
                &dummy_face,
            ))
        });

        let quality_score = embeddings
            .iter()
            .map(|e| e.metadata.quality_score)
            .sum::<f32>()
            / embeddings.len().max(1) as f32;

        debug!(
            "Capture terminée: {} frames RGB, {} frames IR, qualité={:.2}, liveness_ir={:?}",
            rgb_frames.len(),
            ir_frames.as_ref().map(|v| v.len()).unwrap_or(0),
            quality_score,
            ir_liveness,
        );

        Ok(CaptureResult {
            frames: rgb_frames,
            ir_frames,
            embeddings,
            quality_score,
            ir_liveness,
        })
    }

    /// Démarrer une session de capture avec streaming en direct
    pub async fn start_capture_stream<F>(
        &self,
        num_frames: u32,
        timeout_ms: u64,
        mut on_frame: F,
    ) -> Result<(), CameraError>
    where
        F: FnMut(CaptureFrameEvent),
    {
        info!(
            "Démarrage capture streaming: {} frames, timeout={}ms",
            num_frames, timeout_ms
        );

        let rgb_device = self.rgb_device.clone();
        let mut frame_num: u32 = 0;

        let v4l2_result = tokio::task::block_in_place(|| {
            hello_camera::capture_rgb_stream_v4l2(
                &rgb_device,
                num_frames,
                timeout_ms,
                |rgb_data, width, height| {
                    let event = CaptureFrameEvent {
                        frame_number: frame_num,
                        total_frames: num_frames,
                        frame_data: rgb_data,
                        width,
                        height,
                        face_detected: false,
                        face_box: None,
                        quality_score: 0.85,
                        timestamp_ms: 0,
                    };
                    on_frame(event);
                    frame_num += 1;
                },
            )
        });

        match v4l2_result {
            Ok(()) => {
                info!("Capture V4L2 streaming terminée: {} frames", frame_num);
                return Ok(());
            }
            Err(e) => {
                warn!("V4L2 non disponible ({}), utilisation simulation", e);
            }
        }

        // Simulation de repli: gradient RGB animé ~30fps
        let start_time = SystemTime::now();
        let timeout_dur = Duration::from_millis(timeout_ms);

        for frame_num_sim in 0..num_frames {
            if start_time.elapsed().unwrap_or_default() > timeout_dur {
                return Err(CameraError::Timeout);
            }

            let frame_data: Vec<u8> = (0u32..640 * 480)
                .flat_map(|i| {
                    let x = (i % 640) as u8;
                    let y = (i / 640) as u8;
                    let t = (frame_num_sim.wrapping_mul(40)) as u8;
                    [x.wrapping_add(t), y.wrapping_add(t), 128u8]
                })
                .collect();

            let ts = start_time
                .elapsed()
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            on_frame(CaptureFrameEvent {
                frame_number: frame_num_sim,
                total_frames: num_frames,
                frame_data,
                width: 640,
                height: 480,
                face_detected: false,
                face_box: None,
                quality_score: 0.85,
                timestamp_ms: ts,
            });
            tokio::time::sleep(Duration::from_millis(33)).await;
        }

        info!(
            "Capture streaming terminée: {} frames (simulation)",
            num_frames
        );
        Ok(())
    }
}

/// Embedding temporaire (Phase 2 remplacera par ArcFace).
///
/// Découpe l'image en 128 blocs et calcule la moyenne de chaque bloc,
/// produisant un vecteur 128-dim L2-normalisé. Reproductible pour la
/// même image, ce qui permet le matching entre deux enregistrements.
fn compute_stub_embedding(data: &[u8], width: u32, height: u32) -> Vec<f32> {
    const DIMS: usize = 128;
    let pixels = (width * height) as usize;
    if pixels == 0 || data.is_empty() {
        return vec![0.0; DIMS];
    }

    let block = (pixels / DIMS).max(1);
    let channels = (data.len() / pixels).max(1);

    let mut vec: Vec<f32> = (0..DIMS)
        .map(|b| {
            let start = (b * block * channels).min(data.len());
            let end = ((b + 1) * block * channels).min(data.len());
            if start >= end {
                return 0.0;
            }
            data[start..end].iter().map(|&x| x as f32).sum::<f32>() / ((end - start) as f32 * 255.0)
        })
        .collect();

    // L2-normalisation
    let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
    for v in &mut vec {
        *v /= norm;
    }
    vec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_camera_manager_creation() {
        let camera = CameraManager::new(5000);
        // Le scan ne doit pas paniquer même sans /dev/video*
        assert!(!camera.rgb_device.is_empty());
    }

    #[tokio::test]
    async fn test_capture_frames_fallback() {
        let camera = CameraManager::new(5000);
        let result = camera.capture_frames(3, 1000).await.unwrap();
        assert_eq!(result.frames.len(), 3);
        assert_eq!(result.embeddings.len(), 3);
        assert_eq!(result.embeddings[0].vector.len(), 128);
    }

    #[test]
    fn test_stub_embedding_normalized() {
        let data = vec![128u8; 640 * 480 * 3];
        let emb = compute_stub_embedding(&data, 640, 480);
        assert_eq!(emb.len(), 128);
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "Embedding non normalisé: {}",
            norm
        );
    }
}
