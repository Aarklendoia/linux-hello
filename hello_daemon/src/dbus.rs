//! Surface D-Bus pour FaceAuthDaemon
//!
//! Wrapper qui expose les opérations du daemon via D-Bus

use crate::dbus_interface::{DeleteFaceRequest, RegisterFaceRequest, VerifyRequest};
use crate::dbus_signals::StreamingSignalEmitter;
use crate::FaceAuthDaemon;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use zbus::{interface, Connection};

/// Wrapper D-Bus autour du daemon
pub struct FaceAuthInterface {
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    signal_emitter: Option<Arc<StreamingSignalEmitter>>,
    version: String,
    storage_path: String,
}

impl FaceAuthInterface {
    /// Créer une nouvelle interface sans émetteur de signaux (compatible arrière)
    pub fn new(daemon: FaceAuthDaemon) -> Self {
        let storage_path = daemon.config().storage_path.to_string_lossy().into_owned();
        Self {
            daemon: Arc::new(RwLock::new(daemon)),
            signal_emitter: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            storage_path,
        }
    }

    /// Créer une nouvelle interface avec émetteur de signaux D-Bus
    pub fn new_with_connection(daemon: FaceAuthDaemon, connection: Connection) -> Self {
        let storage_path = daemon.config().storage_path.to_string_lossy().into_owned();
        let signal_emitter = Arc::new(StreamingSignalEmitter::new(Arc::new(connection)));
        Self {
            daemon: Arc::new(RwLock::new(daemon)),
            signal_emitter: Some(signal_emitter),
            version: env!("CARGO_PKG_VERSION").to_string(),
            storage_path,
        }
    }
}

#[interface(name = "com.linuxhello.FaceAuth")]
impl FaceAuthInterface {
    /// Enregistrer un nouveau visage pour un utilisateur
    ///
    /// # Arguments
    /// * `request_json` - JSON string de RegisterFaceRequest
    ///
    /// # Returns
    /// JSON string de RegisterFaceResponse ou erreur
    pub async fn register_face(&self, request_json: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: register_face");

        let request: RegisterFaceRequest = match serde_json::from_str(request_json) {
            Ok(r) => r,
            Err(e) => {
                error!("JSON parse error: {}", e);
                return Err(zbus::fdo::Error::Failed(format!("JSON parse error: {}", e)));
            }
        };

        let daemon = self.daemon.write().await;
        let response = daemon.register_face(request).await;

        match response {
            Ok(response_json) => {
                info!("register_face succeeded");
                Ok(response_json)
            }
            Err(e) => {
                error!("register_face failed: {}", e);
                Err(zbus::fdo::Error::Failed(e.to_string()))
            }
        }
    }

    /// Supprimer un ou tous les visages
    ///
    /// # Arguments
    /// * `request_json` - JSON string de DeleteFaceRequest
    pub async fn delete_face(&self, request_json: &str) -> zbus::fdo::Result<()> {
        debug!("D-Bus call: delete_face");

        let request: DeleteFaceRequest = match serde_json::from_str(request_json) {
            Ok(r) => r,
            Err(e) => {
                error!("JSON parse error: {}", e);
                return Err(zbus::fdo::Error::Failed(format!("JSON parse error: {}", e)));
            }
        };

        let daemon = self.daemon.write().await;
        let response = daemon.delete_face(request).await;

        match response {
            Ok(_) => {
                info!("delete_face succeeded");
                Ok(())
            }
            Err(e) => {
                error!("delete_face failed: {}", e);
                Err(zbus::fdo::Error::Failed(e.to_string()))
            }
        }
    }

    /// Vérifier l'identité d'un utilisateur
    ///
    /// # Arguments
    /// * `request_json` - JSON string de VerifyRequest
    ///
    /// # Returns
    /// JSON string de VerifyResult
    pub async fn verify(&self, request_json: &str) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: verify");

        let request: VerifyRequest = match serde_json::from_str(request_json) {
            Ok(r) => r,
            Err(e) => {
                error!("JSON parse error: {}", e);
                return Err(zbus::fdo::Error::Failed(format!("JSON parse error: {}", e)));
            }
        };

        let daemon = self.daemon.write().await;
        let result = daemon.verify(request).await;

        match result {
            Ok(result) => {
                let result_json = match serde_json::to_string(&result) {
                    Ok(j) => j,
                    Err(e) => {
                        error!("JSON serialize error: {}", e);
                        return Err(zbus::fdo::Error::Failed(e.to_string()));
                    }
                };
                info!("verify succeeded");
                Ok(result_json)
            }
            Err(e) => {
                error!("verify failed: {}", e);
                Err(zbus::fdo::Error::Failed(e.to_string()))
            }
        }
    }

    /// Lister les visages enregistrés pour un utilisateur
    ///
    /// # Arguments
    /// * `user_id` - UID de l'utilisateur
    ///
    /// # Returns
    /// JSON array de FaceRecord
    pub async fn list_faces(&self, user_id: u32) -> zbus::fdo::Result<String> {
        debug!("D-Bus call: list_faces for user_id={}", user_id);

        let daemon = self.daemon.write().await;
        let result = daemon.list_faces(user_id).await;

        match result {
            Ok(faces_json) => {
                info!("list_faces succeeded");
                Ok(faces_json)
            }
            Err(e) => {
                error!("list_faces failed: {}", e);
                Err(zbus::fdo::Error::Failed(e.to_string()))
            }
        }
    }

    /// Test de connexion
    pub async fn ping(&self) -> zbus::fdo::Result<String> {
        Ok("pong".to_string())
    }

    /// Démarrer une session de capture en streaming avec émission de signaux
    ///
    /// Émet des signaux D-Bus `CaptureProgress` pour chaque frame capturée.
    /// La GUI s'abonne à ces signaux pour afficher la preview en direct.
    ///
    /// # Arguments
    /// * `user_id` - UID de l'utilisateur qui s'enregistre
    /// * `num_frames` - Nombre de frames à capturer (30 par défaut)
    /// * `timeout_ms` - Timeout en millisecondes (120000 par défaut = 2 minutes)
    ///
    /// # Returns
    /// "OK" si la capture a démarré avec succès, ou erreur
    ///
    /// # D-Bus Signal Emitted
    /// `CaptureProgress(event_json: &str)` - Émis pour chaque frame
    pub async fn start_capture_stream(
        &self,
        user_id: u32,
        num_frames: u32,
        timeout_ms: u64,
    ) -> zbus::fdo::Result<String> {
        debug!(
            "D-Bus call: start_capture_stream user_id={} num_frames={} timeout={}ms",
            user_id, num_frames, timeout_ms
        );

        info!(
            "Démarrage streaming capture: user_id={}, {} frames",
            user_id, num_frames
        );

        // Utiliser le camera manager pour capturer en streaming
        let daemon = self.daemon.read().await;
        let camera_manager = daemon.camera_manager();

        // Cloner l'émetteur de signaux pour utiliser dans la closure
        let signal_emitter = self.signal_emitter.clone();

        // Capturer les frames avec callback qui émet les signaux
        let result = camera_manager
            .start_capture_stream(num_frames, timeout_ms, move |event| {
                // Si émetteur disponible, émettre le signal D-Bus
                if let Some(emitter) = &signal_emitter {
                    let emitter_clone = emitter.clone();
                    let event_clone = event.clone();

                    // Utiliser tokio::spawn pour ne pas bloquer la boucle de capture
                    tokio::spawn(async move {
                        if let Err(e) = emitter_clone.emit_capture_progress(&event_clone).await {
                            error!("Erreur émission signal: {}", e);
                        }

                        // Exporter la frame pour affichage GUI (chaque 5ème frame pour réduire I/O)
                        if event_clone.frame_number % 5 == 0 {
                            if let Err(e) = crate::preview::export_preview_frame(
                                &event_clone.frame_data,
                                event_clone.width,
                                event_clone.height,
                            ) {
                                debug!("Erreur export preview: {}", e);
                            }
                        }
                    });
                } else {
                    // Fallback si pas d'émetteur (mode test/debug)
                    debug!(
                        "Frame {}/{} - Pas d'émetteur de signaux",
                        event.frame_number + 1,
                        event.total_frames
                    );
                }
            })
            .await;

        drop(daemon); // Libérer le lock

        match result {
            Ok(_) => {
                info!("start_capture_stream succeeded");

                // Émettre le signal de fin
                if let Some(emitter) = &self.signal_emitter {
                    if let Err(e) = emitter.emit_capture_completed(user_id).await {
                        error!("Erreur émission CaptureCompleted: {}", e);
                    }
                }

                Ok("OK".to_string())
            }
            Err(e) => {
                error!("start_capture_stream failed: {}", e);

                // Émettre le signal d'erreur
                if let Some(emitter) = &self.signal_emitter {
                    let error_msg = format!("{}", e);
                    if let Err(e) = emitter.emit_capture_error(user_id, &error_msg).await {
                        error!("Erreur émission CaptureError: {}", e);
                    }
                }

                Err(zbus::fdo::Error::Failed(e.to_string()))
            }
        }
    }

    #[zbus(property)]
    pub fn version(&self) -> String {
        self.version.clone()
    }

    /// Vérifier si une caméra est disponible
    #[zbus(property)]
    pub fn camera_available(&self) -> bool {
        // On utilise try_read pour ne pas bloquer
        // En cas d'erreur, on suppose que c'est disponible
        self.daemon
            .try_read()
            .map(|daemon| daemon.is_camera_available())
            .unwrap_or(true)
    }

    /// Mode root ou user
    #[zbus(property)]
    pub fn root_mode(&self) -> bool {
        self.daemon
            .try_read()
            .map(|daemon| daemon.config().root_mode)
            .unwrap_or(false)
    }

    /// Chemin de stockage
    #[zbus(property)]
    pub fn storage_path(&self) -> String {
        self.storage_path.clone()
    }
}
