//! PAM Helper daemon pour contourner l'isolation D-Bus
//!
//! Problème : Le module PAM s'exécute en root et ne peut pas accéder au D-Bus utilisateur.
//! Solution : Helper daemon qui tourne en user et fait la passerelle.
//!
//! Communication : Socket Unix à `/tmp/hello-pam-UID.socket`
//!
//! IMPORTANT : utilise tokio::net::UnixListener (async) car std::net::UnixListener
//! devient non-bloquant quand il est utilisé dans un contexte tokio, ce qui provoque
//! EAGAIN sur les read() côté PAM.

use crate::dbus_interface::VerifyRequest;
use serde::{Deserialize, Serialize};
use std::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tracing::{debug, error, info};

/// Requête PAM via socket
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

/// Démarrer le listener socket PAM (async tokio)
pub async fn start_pam_helper(
    uid: u32,
    daemon: std::sync::Arc<tokio::sync::RwLock<crate::FaceAuthDaemon>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = format!("/tmp/hello-pam-{}.socket", uid);

    // Nettoyer l'ancienne socket
    let _ = fs::remove_file(&socket_path);

    // tokio::net::UnixListener : entièrement async, pas de problème EAGAIN
    let listener = UnixListener::bind(&socket_path)?;
    info!("PAM Helper listening on {}", socket_path);

    // 0o600 : seul le propriétaire (daemon user) et root (PAM module) peuvent se connecter.
    // Root bypasse les permissions Unix, donc 0o600 suffit pour le flux normal.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
    }

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let daemon_clone = daemon.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_pam_request(stream, daemon_clone).await {
                            error!("PAM helper error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("PAM helper accept error: {}", e);
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Envoyer une réponse d'échec et fermer le stream.
async fn reject(
    mut stream: tokio::net::UnixStream,
    reason: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let response = PamHelperResponse::Failure {
        reason: reason.to_string(),
    };
    let json = serde_json::to_string(&response)?;
    stream.write_all(json.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

/// Traiter une connexion PAM entrante
async fn handle_pam_request(
    mut stream: tokio::net::UnixStream,
    daemon: std::sync::Arc<tokio::sync::RwLock<crate::FaceAuthDaemon>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Récupérer l'UID du processus connectant avant de lire quoi que ce soit.
    // Cela évite qu'un attaquant forge un user_id différent du sien.
    #[cfg(unix)]
    let peer_uid: Option<u32> = stream.peer_cred().ok().map(|c| c.uid());
    #[cfg(not(unix))]
    let peer_uid: Option<u32> = None;

    // Lire tout ce que le client envoie (il fait shutdown(Write) après)
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;

    if buf.is_empty() {
        return Ok(());
    }

    let request_json = String::from_utf8(buf)?;
    debug!("PAM Helper reçu: {}", request_json);

    let req: PamHelperRequest = serde_json::from_str(&request_json)?;

    // Valider le pair : seuls root (uid=0) et l'utilisateur cible peuvent demander
    // une vérification. Empêche qu'un processus tiers usurpe un autre user_id.
    if let Some(uid) = peer_uid {
        if uid != 0 && uid != req.user_id {
            error!(
                "PAM helper: connexion refusée — peer uid={} demande user_id={}",
                uid, req.user_id
            );
            return reject(stream, "Unauthorized: peer uid does not match requested user_id").await;
        }
    }

    let verify_req = VerifyRequest {
        user_id: req.user_id,
        context: req.context,
        timeout_ms: req.timeout_ms,
    };

    // Appeler le daemon (timeout = timeout demandé + 1s de marge)
    let timeout = std::time::Duration::from_millis(verify_req.timeout_ms + 1000);
    let daemon_guard = daemon.read().await;
    let result = tokio::time::timeout(timeout, daemon_guard.verify(verify_req)).await;
    drop(daemon_guard);

    let response = match result {
        Ok(Ok(crate::dbus_interface::VerifyResult::Success {
            face_id,
            similarity_score,
        })) => PamHelperResponse::Success {
            face_id,
            similarity_score,
        },
        Ok(Ok(_)) => PamHelperResponse::Failure {
            reason: "Face not recognized".to_string(),
        },
        Ok(Err(e)) => PamHelperResponse::Failure {
            reason: e.to_string(),
        },
        Err(_) => PamHelperResponse::Failure {
            reason: "Timeout".to_string(),
        },
    };

    let response_json = serde_json::to_string(&response)?;
    stream.write_all(response_json.as_bytes()).await?;
    stream.shutdown().await?;

    Ok(())
}
