//! PAM Helper daemon pour contourner l'isolation D-Bus
//!
//! Problème : Le module PAM s'exécute en root et ne peut pas accéder au D-Bus utilisateur.
//! Solution : Helper daemon qui tourne en user et fait la passerelle.
//!
//! Communication : Socket Unix à `/tmp/hello-pam-UID.socket`

use crate::dbus_interface::VerifyRequest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use tracing::{debug, error, info};

/// Port helper pour la requête PAM via socket
#[derive(Debug, Serialize, Deserialize)]
pub struct PamHelperRequest {
    pub user_id: u32,
    pub context: String,
    pub timeout_ms: u64,
}

/// Réponse du helper
#[derive(Debug, Serialize, Deserialize)]
pub enum PamHelperResponse {
    Success {
        face_id: String,
        similarity_score: f32,
    },
    Failure {
        reason: String,
    },
}

/// Socket de communication PAM helper
pub async fn start_pam_helper(
    uid: u32,
    daemon: std::sync::Arc<tokio::sync::RwLock<crate::FaceAuthDaemon>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = format!("/tmp/hello-pam-{}.socket", uid);

    // Nettoyer l'ancienne socket
    let _ = fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path)?;
    info!("PAM Helper listening on {}", socket_path);

    // Définir les permissions pour que root puisse accéder
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o666))?;
    }

    // Accepter les connexions
    tokio::spawn(async move {
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let daemon_clone = daemon.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_pam_request(stream, daemon_clone).await {
                            error!("PAM helper error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Traiter une requête PAM
async fn handle_pam_request(
    mut stream: UnixStream,
    daemon: std::sync::Arc<tokio::sync::RwLock<crate::FaceAuthDaemon>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Lire la requête (JSON)
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf)?;

    if n == 0 {
        return Ok(());
    }

    let request_json = String::from_utf8(buf[..n].to_vec())?;
    debug!("PAM Helper received request: {}", request_json);

    let req: PamHelperRequest = serde_json::from_str(&request_json)?;

    // Créer VerifyRequest
    let verify_req = VerifyRequest {
        user_id: req.user_id,
        context: req.context,
        timeout_ms: req.timeout_ms,
    };

    // Appeler le daemon
    let daemon_guard = daemon.read().await;
    let result = daemon_guard.verify(verify_req).await;
    drop(daemon_guard);

    // Créer la réponse
    let response = match result {
        Ok(crate::dbus_interface::VerifyResult::Success {
            face_id,
            similarity_score,
        }) => PamHelperResponse::Success {
            face_id,
            similarity_score,
        },
        Ok(_) => PamHelperResponse::Failure {
            reason: "Face not recognized".to_string(),
        },
        Err(e) => PamHelperResponse::Failure {
            reason: e.to_string(),
        },
    };

    // Envoyer la réponse
    let response_json = serde_json::to_string(&response)?;
    stream.write_all(response_json.as_bytes())?;
    stream.flush()?;

    Ok(())
}
