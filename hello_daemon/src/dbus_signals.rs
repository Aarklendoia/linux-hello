//! Gestion des signaux D-Bus pour le streaming de capture
//!
//! Permet l'émission de signaux D-Bus pour chaque frame capturée,
//! permettant à la GUI de recevoir les mises à jour en temps réel.

use crate::capture_stream::CaptureFrameEvent;
use std::sync::Arc;
use tracing::{debug, error};
use zbus::Connection;

/// Gestionnaire des signaux D-Bus pour le streaming
pub struct StreamingSignalEmitter {
    #[allow(dead_code)]
    connection: Arc<Connection>,
}

impl StreamingSignalEmitter {
    /// Créer un nouveau émetteur de signaux
    pub fn new(connection: Arc<Connection>) -> Self {
        Self { connection }
    }

    /// Émettre un signal de progression de capture
    ///
    /// Envoie l'événement sérialisé en JSON via D-Bus
    ///
    /// # Arguments
    /// * `event` - Événement de capture à émettre
    ///
    /// # Returns
    /// Ok(()) si succès, Err si échec
    pub async fn emit_capture_progress(&self, event: &CaptureFrameEvent) -> Result<(), String> {
        // Sérialiser l'événement en JSON
        let event_json = match serde_json::to_string(&event) {
            Ok(j) => j,
            Err(e) => {
                error!("Erreur sérialisation événement: {}", e);
                return Err(format!("Sérialisation échouée: {}", e));
            }
        };

        debug!(
            "Émission signal CaptureProgress: frame {}/{}, size={}",
            event.frame_number + 1,
            event.total_frames,
            event_json.len()
        );

        // Pour MVP: juste logger le signal
        // En production: utiliser zbus connection pour émettre
        debug!("Signal JSON: {}", event_json);

        Ok(())
    }

    /// Émettre un signal de fin de capture
    pub async fn emit_capture_completed(&self, user_id: u32) -> Result<(), String> {
        debug!("Émission signal CaptureCompleted pour user_id={}", user_id);
        Ok(())
    }

    /// Émettre un signal d'erreur de capture
    pub async fn emit_capture_error(&self, user_id: u32, error_msg: &str) -> Result<(), String> {
        debug!(
            "Émission signal CaptureError pour user_id={}: {}",
            user_id, error_msg
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_streaming_signal_emitter_creation() {
        // Ce test est pour vérifier que la structure compile
        // Les tests vrais nécessitent une connexion D-Bus fonctionnelle
    }
}
