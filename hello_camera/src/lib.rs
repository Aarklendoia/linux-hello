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
    use tracing::info;

    /// Backend V4L2 réel avec accès direct aux devices Linux
    pub struct V4L2Camera {
        config: CameraConfig,
        device: Option<v4l::Device>,
        is_open: bool,
        start_time: Instant,
        stream_initialized: bool,
    }

    impl V4L2Camera {
        pub fn new(config: CameraConfig) -> Self {
            Self {
                config,
                device: None,
                is_open: false,
                start_time: Instant::now(),
                stream_initialized: false,
            }
        }

        /// Ouvrir le device V4L2 et le configurer pour la capture
        fn open_device(&mut self) -> Result<(), CameraError> {
            use v4l::video::Capture;

            // Ouvrir le device V4L2
            let dev = v4l::Device::with_path(&self.config.device_path).map_err(|e| {
                CameraError::OpenFailed(format!(
                    "Impossible d'ouvrir {}: {}",
                    self.config.device_path, e
                ))
            })?;

            // Obtenir le format courant et l'adapter
            let mut format = dev
                .format()
                .map_err(|e| CameraError::OpenFailed(format!("Erreur lecture format: {}", e)))?;

            // Configurer la résolution
            format.width = self.config.width;
            format.height = self.config.height;

            // Choisir le format selon les préférences
            match self.config.preferred_format {
                FrameFormat::Rgb8 => {
                    // RGB24 (format standard V4L2: R,G,B,R,G,B...)
                    format.fourcc = v4l::format::FourCC::new(b"RGB3");
                }
                FrameFormat::Gray8 => {
                    // Format grayscale
                    format.fourcc = v4l::format::FourCC::new(b"GREY");
                }
                FrameFormat::MjPeg => {
                    // Format MJPEG (souvent plus efficace)
                    format.fourcc = v4l::format::FourCC::new(b"MJPG");
                }
            }

            // Appliquer la configuration
            dev.set_format(&format).map_err(|e| {
                CameraError::OpenFailed(format!("Erreur configuration format V4L2: {}", e))
            })?;

            info!(
                "V4L2 device ouvert et configuré: {} ({}x{} @ {}fps)",
                self.config.device_path, self.config.width, self.config.height, self.config.fps
            );

            self.device = Some(dev);
            Ok(())
        }
    }

    impl CameraBackend for V4L2Camera {
        fn open(&mut self) -> Result<(), CameraError> {
            self.open_device()?;
            self.is_open = true;
            self.start_time = Instant::now();

            info!(
                "V4L2 caméra ouverte: {}x{} at {}",
                self.config.width, self.config.height, self.config.device_path
            );
            Ok(())
        }

        fn close(&mut self) -> Result<(), CameraError> {
            self.device = None;
            self.stream_initialized = false;
            self.is_open = false;
            info!("V4L2 caméra fermée");
            Ok(())
        }

        fn capture(&mut self, _timeout_ms: u64) -> Result<Frame, CameraError> {
            use v4l::buffer::Type;
            use v4l::io::traits::CaptureStream;

            if !self.is_open || self.device.is_none() {
                return Err(CameraError::CaptureFailed("Caméra non ouverte".to_string()));
            }

            let dev = self.device.as_ref().unwrap();

            // Créer un stream mmap à chaque capture (approche simple mais fonctionnelle)
            // Dans une implémentation optimisée, on voudrait stocker ceci
            // mais avec les génériques et lifetimes de v4l c'est complexe
            let mut stream = v4l::io::mmap::Stream::with_buffers(dev, Type::VideoCapture, 4)
                .map_err(|e| {
                    CameraError::CaptureFailed(format!("Erreur création stream: {}", e))
                })?;

            // Capturer une frame
            match stream.next() {
                Ok((buf, _meta)) => {
                    let timestamp_ms = self.start_time.elapsed().as_millis() as u64;

                    Ok(Frame {
                        data: buf.to_vec(),
                        width: self.config.width,
                        height: self.config.height,
                        format: self.config.preferred_format,
                        timestamp_ms,
                    })
                }
                Err(e) => Err(CameraError::CaptureFailed(format!(
                    "Erreur capture V4L2: {}",
                    e
                ))),
            }
        }

        fn pending_frames(&self) -> usize {
            if self.is_open {
                1
            } else {
                0
            }
        }

        fn flush_buffers(&mut self) -> Result<(), CameraError> {
            // Drainer les anciens buffers en capturant et jetant quelques frames
            // Note: avec l'approche mmap, c'est plus simple
            if self.is_open {
                // Essayer de faire quelques captures fast pour vider les buffers
                for _ in 0..3 {
                    let _ = self.capture(100);
                }
            }
            Ok(())
        }

        fn is_open(&self) -> bool {
            self.is_open && self.device.is_some()
        }

        fn backend_name(&self) -> &str {
            "V4L2-Logitech-Brio"
        }
    }
}

/// Créer une caméra avec le backend par défaut disponible
pub fn create_camera(config: CameraConfig) -> Result<Box<dyn CameraBackend>, CameraError> {
    #[cfg(feature = "v4l2")]
    {
        Ok(Box::new(v4l2_backend::V4L2Camera::new(config)))
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
