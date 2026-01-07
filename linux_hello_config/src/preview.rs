//! Module Preview - affichage de la caméra en direct avec pixels crate
//!
//! Responsable de:
//! - Afficher les frames RGB en temps réel
//! - Dessiner la bounding box autour du visage détecté
//! - Afficher la barre de progression
//! - Gérer les animations et transitions

use crate::streaming::{CaptureFrame, FaceBox};

/// Utilitaires d'animation
pub mod animation {
    /// Interpolation linéaire entre deux valeurs
    pub fn lerp(current: f32, target: f32, speed: f32) -> f32 {
        if (current - target).abs() < 0.001 {
            target
        } else {
            current + (target - current) * speed
        }
    }

    /// Easing: ease-out (quartic)
    pub fn ease_out_quad(t: f32) -> f32 {
        1.0 - (1.0 - t) * (1.0 - t)
    }

    /// Clamp value between 0.0 and 1.0
    pub fn clamp_01(value: f32) -> f32 {
        value.max(0.0).min(1.0)
    }
}

/// État du preview
pub struct PreviewState {
    pub current_frame: Option<CaptureFrame>,
    pub width: u32,
    pub height: u32,
}

impl PreviewState {
    /// Créer un nouveau state de preview
    pub fn new() -> Self {
        Self {
            current_frame: None,
            width: 640,
            height: 480,
        }
    }

    /// Mettre à jour avec une nouvelle frame
    pub fn update_frame(&mut self, frame: CaptureFrame) {
        self.width = frame.width;
        self.height = frame.height;
        self.current_frame = Some(frame);
    }

    /// Obtenir le pourcentage de progression
    pub fn progress_percent(&self) -> f32 {
        if let Some(ref frame) = self.current_frame {
            if frame.total_frames == 0 {
                0.0
            } else {
                (frame.frame_number as f32 + 1.0) / frame.total_frames as f32
            }
        } else {
            0.0
        }
    }

    /// Obtenir le texte de progression
    pub fn progress_text(&self) -> String {
        if let Some(ref frame) = self.current_frame {
            format!("{}/{} frames", frame.frame_number + 1, frame.total_frames)
        } else {
            "0/0 frames".to_string()
        }
    }

    /// Obtenir le texte du statut de détection
    pub fn detection_status(&self) -> String {
        if let Some(ref frame) = self.current_frame {
            if frame.face_detected {
                format!(
                    "✓ Visage détecté (confiance: {:.1}%)",
                    frame
                        .face_box
                        .as_ref()
                        .map(|b| b.confidence * 100.0)
                        .unwrap_or(0.0)
                )
            } else {
                "⚠ Aucun visage détecté".to_string()
            }
        } else {
            "En attente de capture...".to_string()
        }
    }

    /// Obtenir les données RGB24 prêtes à afficher
    pub fn get_display_data(&self) -> Option<Vec<u8>> {
        self.current_frame.as_ref().map(|frame| {
            let mut data = frame.frame_data.clone();
            self.draw_bounding_box(&mut data);
            data
        })
    }

    /// Dessiner un bounding box sur les données RGB
    pub fn draw_bounding_box(&self, frame_data: &mut [u8]) {
        if let Some(ref frame) = self.current_frame {
            if let Some(ref face_box) = frame.face_box {
                self.draw_box_rect(frame_data, face_box, frame.width);
            }
        }
    }

    /// Dessiner un rectangle vert autour du visage
    fn draw_box_rect(&self, frame_data: &mut [u8], face_box: &FaceBox, width: u32) {
        // Couleur verte (RGB)
        let green_r = 0;
        let green_g = 255;
        let green_b = 0;
        let thickness = 2;

        // Limites du box
        let left = face_box.x as usize;
        let top = face_box.y as usize;
        let right = std::cmp::min(face_box.x + face_box.width, width) as usize;
        let bottom = std::cmp::min(face_box.y + face_box.height, 480) as usize;

        // Dessiner les lignes (simplifié: juste marquer les coins)
        for y in top..std::cmp::min(top + thickness as usize, bottom) {
            for x in left..right {
                let idx = (y * width as usize + x) * 3;
                if idx + 2 < frame_data.len() {
                    frame_data[idx] = green_r;
                    frame_data[idx + 1] = green_g;
                    frame_data[idx + 2] = green_b;
                }
            }
        }

        // Bottom line
        for y in (bottom.saturating_sub(thickness as usize))..bottom {
            for x in left..right {
                let idx = (y * width as usize + x) * 3;
                if idx + 2 < frame_data.len() {
                    frame_data[idx] = green_r;
                    frame_data[idx + 1] = green_g;
                    frame_data[idx + 2] = green_b;
                }
            }
        }

        // Left line
        for y in top..bottom {
            for x in left..std::cmp::min(left + thickness as usize, right) {
                let idx = (y * width as usize + x) * 3;
                if idx + 2 < frame_data.len() {
                    frame_data[idx] = green_r;
                    frame_data[idx + 1] = green_g;
                    frame_data[idx + 2] = green_b;
                }
            }
        }

        // Right line
        for y in top..bottom {
            for x in (right.saturating_sub(thickness as usize))..right {
                let idx = (y * width as usize + x) * 3;
                if idx + 2 < frame_data.len() {
                    frame_data[idx] = green_r;
                    frame_data[idx + 1] = green_g;
                    frame_data[idx + 2] = green_b;
                }
            }
        }
    }
}

impl Default for PreviewState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_state_creation() {
        let state = PreviewState::new();
        assert_eq!(state.width, 640);
        assert_eq!(state.height, 480);
        assert!(state.current_frame.is_none());
    }

    #[test]
    fn test_progress_percent_empty() {
        let state = PreviewState::new();
        assert_eq!(state.progress_percent(), 0.0);
    }

    #[test]
    fn test_progress_text_format() {
        let state = PreviewState::new();
        assert_eq!(state.progress_text(), "0/0 frames");
    }

    #[test]
    fn test_detection_status() {
        let state = PreviewState::new();
        assert!(state.detection_status().contains("En attente"));
    }

    #[test]
    fn test_get_display_data_with_frame() {
        use crate::streaming::{CaptureFrame, FaceBox};

        let mut state = PreviewState::new();
        let frame_data = vec![255, 0, 0].repeat(640 * 480); // Red frame
        let face_box = FaceBox {
            x: 100,
            y: 100,
            width: 50,
            height: 50,
            confidence: 0.95,
        };

        let frame = CaptureFrame {
            frame_number: 0,
            total_frames: 30,
            frame_data,
            width: 640,
            height: 480,
            face_detected: true,
            face_box: Some(face_box),
            quality_score: 0.9,
            timestamp_ms: 1000,
        };

        state.update_frame(frame);
        let display_data = state.get_display_data();

        assert!(display_data.is_some());
        let data = display_data.unwrap();
        assert_eq!(data.len(), 640 * 480 * 3);
    }
}

#[cfg(test)]
mod animation_tests {
    use super::animation::*;

    #[test]
    fn test_lerp_interpolation() {
        let result = lerp(0.0, 1.0, 0.5);
        assert!((result - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_lerp_at_target() {
        let result = lerp(0.5, 0.5, 0.1);
        assert!((result - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_ease_out_quad() {
        let result = ease_out_quad(0.5);
        // ease_out_quad(0.5) = 1 - (1-0.5)^2 = 1 - 0.25 = 0.75
        assert!((result - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_clamp_01_bounds() {
        assert_eq!(clamp_01(0.5), 0.5);
        assert_eq!(clamp_01(-0.1), 0.0);
        assert_eq!(clamp_01(1.5), 1.0);
    }
}
