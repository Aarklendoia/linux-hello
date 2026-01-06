//! Abstraction d'accès caméra multi-backend (V4L2, PipeWire)

use std::sync::Arc;
use thiserror::Error;

/// Erreurs caméra
#[derive(Debug, Error)]
pub enum CameraError {
    #[error("Caméra non disponible: {0}")]
    NotAvailable(String),

    #[error("Erreur d'ouverture: {0}")]
    OpenFailed(String),

    #[error("Erreur de capture: {0}")]
    CaptureFailed(String),

    #[error("Format non supporté: {0}")]
    UnsupportedFormat(String),

    #[error("Timeout de capture")]
    CaptureTimeout,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Format de frame supporté
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFormat {
    /// RGB 8 bits par canal
    Rgb8,
    /// Grayscale 8 bits
    Gray8,
    /// MJPEG compressé
    MjPeg,
}

impl FrameFormat {
    pub fn channels(&self) -> u32 {
        match self {
            FrameFormat::Rgb8 => 3,
            FrameFormat::Gray8 => 1,
            FrameFormat::MjPeg => 3, // décodé en RGB
        }
    }
}

/// Description d'une frame capturée
#[derive(Debug, Clone)]
pub struct Frame {
    /// Données brutes des pixels
    pub data: Vec<u8>,

    /// Largeur en pixels
    pub width: u32,

    /// Hauteur en pixels
    pub height: u32,

    /// Format de la frame
    pub format: FrameFormat,

    /// Timestamp de capture (ms depuis début)
    pub timestamp_ms: u64,
}

impl Frame {
    /// Retourner le nombre de canaux
    pub fn channels(&self) -> u32 {
        self.format.channels()
    }

    /// Vérifier que les données correspondent aux dimensions
    pub fn validate(&self) -> Result<(), CameraError> {
        let expected_size = match self.format {
            FrameFormat::Rgb8 => (self.width * self.height * 3) as usize,
            FrameFormat::Gray8 => (self.width * self.height) as usize,
            FrameFormat::MjPeg => self.data.len(), // taille variable
        };

        if self.format != FrameFormat::MjPeg && self.data.len() != expected_size {
            return Err(CameraError::CaptureFailed(format!(
                "Mismatch taille frame: attendu {}, got {}",
                expected_size,
                self.data.len()
            )));
        }
        Ok(())
    }
}

/// Configuration caméra
#[derive(Debug, Clone)]
pub struct CameraConfig {
    /// Chemin du device (ex: /dev/video0)
    pub device_path: String,

    /// Largeur souhaitée
    pub width: u32,

    /// Hauteur souhaitée
    pub height: u32,

    /// Format préféré
    pub preferred_format: FrameFormat,

    /// FPS ciblés
    pub fps: u32,

    /// Timeout de capture (ms)
    pub capture_timeout_ms: u64,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            device_path: "/dev/video0".to_string(),
            width: 640,
            height: 480,
            preferred_format: FrameFormat::Rgb8,
            fps: 30,
            capture_timeout_ms: 5000,
        }
    }
}

/// Trait pour caméra générique
pub trait CameraBackend: Send + Sync {
    /// Démarrer la caméra
    fn open(&mut self) -> Result<(), CameraError>;

    /// Arrêter la caméra
    fn close(&mut self) -> Result<(), CameraError>;

    /// Capturer une frame (bloquant)
    fn capture(&mut self, timeout_ms: u64) -> Result<Frame, CameraError>;

    /// Nombre de frames en attente
    fn pending_frames(&self) -> usize;

    /// Vider le buffer (utile avant vérification)
    fn flush_buffers(&mut self) -> Result<(), CameraError>;

    /// Vérifier que la caméra est ouverte
    fn is_open(&self) -> bool;

    /// Nom du backend
    fn backend_name(&self) -> &str;
}

/// Handle partagé pour une caméra
pub type SharedCamera = Arc<parking_lot::Mutex<Box<dyn CameraBackend>>>;

// ============================================================================
// Implémentation V4L2
// ============================================================================

#[cfg(feature = "v4l2")]
pub mod v4l2_backend {
    use super::*;
    use std::time::Instant;

    /// Backend V4L2 simplifié - stub pour compatibilité
    pub struct V4L2Camera {
        config: CameraConfig,
        is_open: bool,
        start_time: Instant,
    }

    impl V4L2Camera {
        pub fn new(config: CameraConfig) -> Self {
            Self {
                config,
                is_open: false,
                start_time: Instant::now(),
            }
        }
    }

    impl CameraBackend for V4L2Camera {
        fn open(&mut self) -> Result<(), CameraError> {
            // Vérifier que le device existe
            std::fs::metadata(&self.config.device_path).map_err(|e| {
                CameraError::OpenFailed(format!(
                    "Failed to access {}: {}",
                    self.config.device_path, e
                ))
            })?;

            tracing::info!(
                "V4L2 camera opened: {}x{} at {}",
                self.config.width,
                self.config.height,
                self.config.device_path
            );

            self.is_open = true;
            self.start_time = Instant::now();
            Ok(())
        }

        fn close(&mut self) -> Result<(), CameraError> {
            self.is_open = false;
            Ok(())
        }

        fn capture(&mut self, _timeout_ms: u64) -> Result<Frame, CameraError> {
            if !self.is_open {
                return Err(CameraError::CaptureFailed("Caméra non ouverte".to_string()));
            }

            // Stub: retourner une frame de test colorée
            let frame_size = (self.config.width * self.config.height * 3) as usize;
            let mut data = vec![0u8; frame_size];

            // Créer une image de test avec des dégradés (simule une vraie caméra)
            for y in 0..self.config.height {
                for x in 0..self.config.width {
                    let idx = ((y * self.config.width + x) * 3) as usize;
                    data[idx] = ((x * 255) / self.config.width) as u8; // R
                    data[idx + 1] = ((y * 255) / self.config.height) as u8; // G
                    data[idx + 2] = 128; // B
                }
            }

            let timestamp_ms = self.start_time.elapsed().as_millis() as u64;

            Ok(Frame {
                data,
                width: self.config.width,
                height: self.config.height,
                format: FrameFormat::Rgb8,
                timestamp_ms,
            })
        }

        fn pending_frames(&self) -> usize {
            0
        }

        fn flush_buffers(&mut self) -> Result<(), CameraError> {
            Ok(())
        }

        fn is_open(&self) -> bool {
            self.is_open
        }

        fn backend_name(&self) -> &str {
            "V4L2-Stub"
        }
    }
}

/// Créer une caméra avec le backend par défaut disponible
pub fn create_camera(config: CameraConfig) -> Result<Box<dyn CameraBackend>, CameraError> {
    #[cfg(feature = "v4l2")]
    {
        return Ok(Box::new(v4l2_backend::V4L2Camera::new(config)));
    }

    #[cfg(not(feature = "v4l2"))]
    {
        Err(CameraError::NotAvailable(
            "Aucun backend caméra compilé".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_format_channels() {
        assert_eq!(FrameFormat::Rgb8.channels(), 3);
        assert_eq!(FrameFormat::Gray8.channels(), 1);
    }

    #[test]
    fn test_camera_config_default() {
        let config = CameraConfig::default();
        assert_eq!(config.width, 640);
        assert_eq!(config.height, 480);
    }
}
