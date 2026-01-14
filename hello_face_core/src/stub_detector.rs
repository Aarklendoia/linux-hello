//! Stub detector rapide pour preview en direct
//!
//! Détecte juste le bounding box sans extraction complète
//! Utilisé pour le streaming live (basse latence)

use super::{FaceDetector, FaceError, FaceRegion};

/// Détecteur stub - simule une détection rapide
/// À remplacer par YOLO ou RetinaFace réel
pub struct StubDetector {
    name: String,
    version: String,
}

impl StubDetector {
    pub fn new() -> Self {
        Self {
            name: "stub-detector".to_string(),
            version: "0.1.0".to_string(),
        }
    }
}

impl Default for StubDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl FaceDetector for StubDetector {
    fn detect(
        &self,
        frame_data: &[u8],
        width: u32,
        height: u32,
        channels: u32,
    ) -> Result<Vec<FaceRegion>, FaceError> {
        // Stub: simuler une détection simple
        // En production: utiliser YOLO, RetinaFace, etc.

        // Validation basique
        if frame_data.is_empty() || width == 0 || height == 0 || channels == 0 {
            return Ok(Vec::new());
        }

        let expected_size = (width * height * channels) as usize;
        if frame_data.len() < expected_size {
            return Err(FaceError::InvalidFrame(format!(
                "Taille frame invalide: {} < {}",
                frame_data.len(),
                expected_size
            )));
        }

        // Stub: détecter un visage au centre (pour test)
        // Simule: détection d'un visage carré au centre 200x200px
        let face_width = (width as f32 * 0.3) as u32;
        let face_height = (height as f32 * 0.4) as u32;
        let face_x = (width - face_width) / 2;
        let face_y = (height - face_height) / 2;

        // Détection basée sur contraste simple (stub)
        let face_detected = self.detect_simple_contrast(
            frame_data,
            width,
            height,
            channels,
            face_x,
            face_y,
            face_width,
            face_height,
        );

        if face_detected {
            Ok(vec![FaceRegion {
                bounding_box: (face_x, face_y, face_width, face_height),
                confidence: 0.85, // stub confidence
                landmarks: vec![],
            }])
        } else {
            Ok(Vec::new())
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn model_version(&self) -> &str {
        &self.version
    }
}

impl StubDetector {
    /// Détection basée sur contraste simple
    #[allow(clippy::too_many_arguments)]
    fn detect_simple_contrast(
        &self,
        frame_data: &[u8],
        _width: u32,
        _height: u32,
        channels: u32,
        face_x: u32,
        face_y: u32,
        face_width: u32,
        face_height: u32,
    ) -> bool {
        // Calculer une moyenne de contraste dans la région supposée du visage
        let mut total = 0u32;
        let mut count = 0u32;

        for y in face_y..face_y + face_height {
            for x in face_x..face_x + face_width {
                let idx = ((y * _width + x) * channels) as usize;

                // Utiliser le canal rouge (ou moyenne)
                if idx < frame_data.len() {
                    total += frame_data[idx] as u32;
                    count += 1;
                }
            }
        }

        if count == 0 {
            return false;
        }

        let avg = total / count;
        // Stub: déterminer si "assez de contraste"
        // Intervalle [50, 200] est considéré comme un visage
        avg > 50 && avg < 200
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_detector_creation() {
        let detector = StubDetector::new();
        assert_eq!(detector.name(), "stub-detector");
    }

    #[test]
    fn test_stub_detector_invalid_frame() {
        let detector = StubDetector::new();
        let result = detector.detect(&[], 0, 0, 3);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_stub_detector_empty_frame() {
        let detector = StubDetector::new();
        let frame = vec![0u8; 640 * 480 * 3];
        let result = detector.detect(&frame, 640, 480, 3).unwrap();
        // Avec tous les pixels noirs, pas de détection
        assert!(result.is_empty());
    }
}
