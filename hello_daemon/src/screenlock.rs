//! Surveillance du verrouillage d'écran et authentification faciale automatique
//!
//! Interroge org.freedesktop.ScreenSaver.GetActive() toutes les 500 ms pour détecter
//! le verrouillage. Quand l'écran se verrouille, déclenche l'auth faciale sans que
//! l'utilisateur appuie sur Entrée. Si le visage est reconnu, déverrouille via loginctl.
//!
//! On utilise le polling plutôt que la souscription au signal ActiveChanged pour éviter
//! une dépendance conflictuelle sur futures-util (pinné à 0.3.32 par sqlx).

use crate::dbus_interface::{VerifyRequest, VerifyResult};
use crate::FaceAuthDaemon;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use zbus::proxy;

/// Proxy pour org.freedesktop.ScreenSaver (bus session, chemin standard KDE)
#[proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver",
    gen_blocking = false
)]
trait ScreenSaver {
    /// Retourne true si l'écran est actuellement verrouillé.
    fn get_active(&self) -> zbus::Result<bool>;
}

/// Démarrer la surveillance du verrouillage d'écran.
///
/// Retourne immédiatement ; la boucle de surveillance tourne dans une tâche tokio dédiée.
pub async fn start_screenlock_watcher(
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    user_id: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session_id = get_session_id();
    info!(
        "Démarrage surveillance écran (uid={}, session={})",
        user_id, session_id
    );

    let connection = zbus::Connection::session().await?;
    let proxy = ScreenSaverProxy::new(&connection).await?;

    // Vérifier que le proxy répond avant de lancer la boucle
    let _ = proxy.get_active().await?;

    tokio::spawn(async move {
        screenlock_loop(proxy, daemon, user_id, session_id).await;
    });

    Ok(())
}

/// Boucle principale : polling toutes les 500 ms, détection des transitions lock/unlock.
async fn screenlock_loop(
    proxy: ScreenSaverProxy<'_>,
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    user_id: u32,
    session_id: String,
) {
    let mut was_active = false;
    // auth_running empêche de lancer plusieurs auths simultanées (ex: verrouillage rapide)
    let mut auth_running = false;

    info!("Surveillance verrouillage d'écran active (polling 500 ms)");

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let is_active = match proxy.get_active().await {
            Ok(v) => v,
            Err(e) => {
                warn!("ScreenSaver.GetActive() error: {} — retry dans 2 s", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        if is_active && !was_active {
            // Transition → verrouillé
            info!("Écran verrouillé détecté → lancement auth faciale automatique");
            was_active = true;

            if !auth_running {
                auth_running = true;
                let daemon_clone = daemon.clone();
                let session_id_clone = session_id.clone();

                tokio::spawn(async move {
                    try_face_unlock(daemon_clone, user_id, &session_id_clone).await;
                });
            }
        } else if !is_active && was_active {
            // Transition → déverrouillé (par visage ou mot de passe)
            info!("Écran déverrouillé");
            was_active = false;
            auth_running = false;
        }
    }
}

/// Tenter une authentification faciale et déverrouiller si succès.
async fn try_face_unlock(
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    user_id: u32,
    session_id: &str,
) {
    // Laisser le temps au lock screen de s'afficher complètement.
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;

    info!(
        "Auth faciale automatique pour uid={} (context=screenlock, timeout=12 s)",
        user_id
    );

    let result = {
        let d = daemon.read().await;
        d.verify(VerifyRequest {
            user_id,
            context: "screenlock".to_string(),
            timeout_ms: 12000,
        })
        .await
    };

    match result {
        Ok(VerifyResult::Success {
            face_id,
            similarity_score,
        }) => {
            info!(
                "Visage reconnu (id={}, score={:.3}) → déverrouillage",
                face_id, similarity_score
            );
            if let Err(e) = unlock_session(session_id).await {
                error!("Échec déverrouillage loginctl: {}", e);
            }
        }
        Ok(VerifyResult::NoEnrollment) => {
            info!(
                "Aucun visage enregistré pour uid={} — pas d'auth automatique",
                user_id
            );
        }
        Ok(other) => {
            info!(
                "Auth faciale non concluante ({}): l'utilisateur peut entrer son mot de passe",
                other
            );
        }
        Err(e) => {
            warn!(
                "Erreur auth faciale: {} — l'utilisateur peut entrer son mot de passe",
                e
            );
        }
    }
}

/// Déverrouiller la session KDE via loginctl.
/// loginctl envoie org.freedesktop.login1.Session.Unlock → kscreenlocker se ferme.
async fn unlock_session(session_id: &str) -> Result<(), String> {
    let output = tokio::process::Command::new("loginctl")
        .args(["unlock-session", session_id])
        .output()
        .await
        .map_err(|e| format!("spawn loginctl: {}", e))?;

    if output.status.success() {
        info!("loginctl unlock-session {} : succès", session_id);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "loginctl exit={}: {}",
            output.status,
            stderr.trim()
        ))
    }
}

/// Obtenir le session ID logind du processus courant.
/// Priorité : $XDG_SESSION_ID → loginctl list-sessions (première session de l'utilisateur).
fn get_session_id() -> String {
    if let Ok(id) = std::env::var("XDG_SESSION_ID") {
        if !id.is_empty() {
            return id;
        }
    }

    // Fallback synchrone au démarrage
    if let Ok(output) = std::process::Command::new("loginctl")
        .args(["--no-legend", "--no-pager", "list-sessions"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let uid = unsafe { libc::getuid() };
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Format : SESSION_ID UID USER SEAT ...
            if parts.len() >= 2 {
                if let Ok(line_uid) = parts[1].parse::<u32>() {
                    if line_uid == uid {
                        return parts[0].to_string();
                    }
                }
            }
        }
    }

    "1".to_string()
}
