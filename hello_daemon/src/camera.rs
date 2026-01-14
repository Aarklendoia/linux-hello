//! Abstraction caméra pour le daemon
//!
//! Fournit une interface simple pour capturer des frames
//! et les passer au moteur de reconnaissance

use crate::capture_stream::CaptureFrameEvent;
use hello_camera::Frame;
use hello_face_core::Embedding;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tracing::{debug, info};

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
    /// Frames capturées
    pub frames: Vec<Frame>,

    /// Embeddings extraits
    pub embeddings: Vec<Embedding>,

    /// Score de qualité moyen
    pub quality_score: f32,
}

/// Gestionnaire caméra pour le daemon
pub struct CameraManager {
    /// Timeout par défaut pour les captures (ms)
    default_timeout_ms: u64,
}

impl CameraManager {
    /// Créer un nouveau gestionnaire caméra
    pub fn new(default_timeout_ms: u64) -> Self {
        Self { default_timeout_ms }
    }

    /// Vérifier si une caméra est disponible
    pub fn is_available(&self) -> bool {
        // Pour MVP: toujours true (implémentation réelle plus tard)
        true
    }

    /// Capturer N frames et extraire les embeddings
    ///
    /// # Arguments
    /// * `num_frames` - Nombre de frames à capturer
    /// * `timeout_ms` - Timeout en millisecondes (0 = utiliser default)
    ///
    /// # Returns
    /// CaptureResult avec frames et embeddings, ou CameraError
    pub async fn capture_frames(
        &self,
        num_frames: u32,
        timeout_ms: u64,
    ) -> Result<CaptureResult, CameraError> {
        let timeout = if timeout_ms == 0 {
            Duration::from_millis(self.default_timeout_ms)
        } else {
            Duration::from_millis(timeout_ms)
        };

        info!(
            "Capture de {} frames avec timeout {:?}",
            num_frames, timeout
        );

        // Pour MVP: génération de données de test
        // En production: utiliser hello_camera pour acquisition réelle
        let mut frames = Vec::new();
        let mut embeddings = Vec::new();

        for i in 0..num_frames {
            // Simulation frame
            let frame = Frame {
                data: vec![0; 1920 * 1080 * 3], // RGB dummy
                width: 1920,
                height: 1080,
                format: hello_camera::FrameFormat::Rgb8,
                timestamp_ms: i as u64 * 100,
            };

            // Simulation extraction embedding
            // En prod: utiliser hello_face_core::FaceDetector + extract_embedding
            let embedding = hello_face_core::Embedding {
                vector: (0..128).map(|j| (i as f32 + j as f32) / 1000.0).collect(),
                metadata: hello_face_core::EmbeddingMetadata {
                    model: "sim_model".to_string(),
                    model_version: "0.1.0".to_string(),
                    extracted_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    quality_score: 0.85,
                },
            };

            frames.push(frame);
            embeddings.push(embedding);

            debug!(
                "Frame {}/{} capturée et embeddings extraits",
                i + 1,
                num_frames
            );
        }

        // Calculer score de qualité moyen
        let quality_score = 0.85; // À implémenter avec vraie logique

        Ok(CaptureResult {
            frames,
            embeddings,
            quality_score,
        })
    }

    /// Capturer une seule frame de test
    pub async fn test_capture(&self) -> Result<Vec<u8>, CameraError> {
        info!("Capture test");

        // Dummy image RGB 640x480
        Ok(vec![0; 640 * 480 * 3])
    }

    /// Démarrer une session de capture avec streaming en direct
    ///
    /// Émet des événements CaptureFrameEvent via callback pour chaque frame capturée.
    /// Idéal pour envoyer via signaux D-Bus à la GUI.
    ///
    /// # Arguments
    /// * `num_frames` - Nombre total de frames à capturer (ex: 30)
    /// * `timeout_ms` - Timeout total en millisecondes
    /// * `on_frame` - Callback appelé pour chaque frame capturée
    ///
    /// # Example
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let camera = hello_daemon::camera::CameraManager::new(5000);
    /// camera.start_capture_stream(30, 120000, |event| {
    ///     println!("Frame {}/{}", event.frame_number, event.total_frames);
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
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

        let start_time = SystemTime::now();
        let timeout = Duration::from_millis(timeout_ms);

        for frame_num in 0..num_frames {
            // Vérifier timeout
            if let Ok(elapsed) = start_time.elapsed() {
                if elapsed > timeout {
                    debug!("Timeout capture streaming");
                    return Err(CameraError::Timeout);
                }
            }

            // Capturer frame (pour MVP: dummy data)
            let frame_data = vec![0; 640 * 480 * 3]; // RGB 640x480

            // Créer événement de capture
            let timestamp_ms = start_time
                .elapsed()
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            let event = CaptureFrameEvent {
                frame_number: frame_num,
                total_frames: num_frames,
                frame_data,
                width: 640,
                height: 480,
                face_detected: false, // Placeholder pour Phase 2
                face_box: None,       // Placeholder pour Phase 2
                quality_score: 0.85,
                timestamp_ms,
            };

            debug!(
                "Capture frame {}/{} à {}ms",
                frame_num + 1,
                num_frames,
                timestamp_ms
            );

            // Émettre l'événement
            on_frame(event);

            // Petit délai pour simulation
            tokio::time::sleep(Duration::from_millis(33)).await; // ~30fps
        }

        info!(
            "Capture streaming terminée: {} frames capturées",
            num_frames
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_camera_manager_available() {
        let camera = CameraManager::new(5000);
        assert!(camera.is_available());
    }

    #[tokio::test]
    async fn test_capture_frames() {
        let camera = CameraManager::new(5000);
        let result = camera.capture_frames(3, 0).await.unwrap();

        assert_eq!(result.frames.len(), 3);
        assert_eq!(result.embeddings.len(), 3);
        assert_eq!(result.embeddings[0].vector.len(), 128);
    }

    #[tokio::test]
    async fn test_start_capture_stream() {
        let camera = CameraManager::new(5000);
        let mut frame_count = 0;

        let result = camera
            .start_capture_stream(5, 10000, |event| {
                // Vérifier structure de l'événement
                assert_eq!(event.total_frames, 5);
                assert_eq!(event.width, 640);
                assert_eq!(event.height, 480);
                assert_eq!(event.frame_data.len(), 640 * 480 * 3);
                frame_count += 1;
            })
            .await;

        assert!(result.is_ok());
        // Note: frame_count ne sera pas accessible ici, juste vérifier que ça compile
    }

    #[tokio::test]
    async fn test_start_capture_stream_collects_frames() {
        use std::sync::{Arc, Mutex};

        let camera = CameraManager::new(5000);
        let frames_captured = Arc::new(Mutex::new(Vec::new()));
        let frames_captured_clone = frames_captured.clone();

        let _ = camera
            .start_capture_stream(3, 10000, move |event| {
                frames_captured_clone
                    .lock()
                    .unwrap()
                    .push(event.frame_number);
            })
            .await;

        let captured = frames_captured.lock().unwrap();
        assert_eq!(captured.len(), 3);
        assert_eq!(captured[0], 0);
        assert_eq!(captured[1], 1);
        assert_eq!(captured[2], 2);
    }
}
