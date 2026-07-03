//! Screen lock monitoring and automatic facial authentication
//!
//! Polls org.freedesktop.ScreenSaver.GetActive() every 500 ms to detect
//! locking. When the screen locks, triggers facial auth without the user
//! having to press Enter. If the face is recognized, unlocks via loginctl.
//!
//! Polling is used instead of subscribing to the ActiveChanged signal to avoid
//! a conflicting dependency on futures-util (pinned to 0.3.32 by sqlx).

use crate::dbus_interface::{VerifyRequest, VerifyResult};
use crate::FaceAuthDaemon;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use zbus::proxy;

/// Proxy for org.freedesktop.ScreenSaver (session bus, standard KDE path)
#[proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver",
    gen_blocking = false
)]
trait ScreenSaver {
    /// Returns true if the screen is currently locked.
    fn get_active(&self) -> zbus::Result<bool>;
}

/// Start monitoring the screen lock.
///
/// Returns immediately; the monitoring loop runs in a dedicated tokio task.
pub async fn start_screenlock_watcher(
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    user_id: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let session_id = get_session_id();
    info!(
        "Starting screen monitoring (uid={}, session={})",
        user_id, session_id
    );

    let connection = zbus::Connection::session().await?;
    let proxy = ScreenSaverProxy::new(&connection).await?;

    // Verify that the proxy responds before starting the loop
    let _ = proxy.get_active().await?;

    tokio::spawn(async move {
        screenlock_loop(proxy, daemon, user_id, session_id).await;
    });

    Ok(())
}

/// Main loop: polling every 500 ms, detecting lock/unlock transitions.
async fn screenlock_loop(
    proxy: ScreenSaverProxy<'_>,
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    user_id: u32,
    session_id: String,
) {
    let mut was_active = false;
    // auth_running prevents launching multiple simultaneous auths (e.g. rapid locking)
    let mut auth_running = false;

    info!("Screen lock monitoring active (polling 500 ms)");

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let is_active = match proxy.get_active().await {
            Ok(v) => v,
            Err(e) => {
                warn!("ScreenSaver.GetActive() error: {} — retrying in 2s", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        if is_active && !was_active {
            // Transition → locked
            info!("Screen lock detected → launching automatic facial auth");
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
            // Transition → unlocked (via face or password)
            info!("Screen unlocked");
            was_active = false;
            auth_running = false;
        }
    }
}

/// Attempt facial authentication and unlock on success.
async fn try_face_unlock(
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    user_id: u32,
    session_id: &str,
) {
    // Give the lock screen time to fully render.
    tokio::time::sleep(tokio::time::Duration::from_millis(1200)).await;

    info!(
        "Automatic facial auth for uid={} (context=screenlock, timeout=12s)",
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
                "Face recognized (id={}, score={:.3}) → unlocking",
                face_id, similarity_score
            );
            if let Err(e) = unlock_session(session_id).await {
                error!("loginctl unlock failed: {}", e);
            }
        }
        Ok(VerifyResult::NoEnrollment) => {
            info!(
                "No face registered for uid={} — no automatic auth",
                user_id
            );
        }
        Ok(other) => {
            info!(
                "Facial auth inconclusive ({}): the user can enter their password",
                other
            );
        }
        Err(e) => {
            warn!(
                "Facial auth error: {} — the user can enter their password",
                e
            );
        }
    }
}

/// Unlock the KDE session via loginctl.
/// loginctl sends org.freedesktop.login1.Session.Unlock → kscreenlocker closes.
async fn unlock_session(session_id: &str) -> Result<(), String> {
    let output = tokio::process::Command::new("loginctl")
        .args(["unlock-session", session_id])
        .output()
        .await
        .map_err(|e| format!("spawn loginctl: {}", e))?;

    if output.status.success() {
        info!("loginctl unlock-session {}: success", session_id);
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

/// Get the logind session ID of the current process.
/// Priority: $XDG_SESSION_ID → loginctl list-sessions (user's first session).
fn get_session_id() -> String {
    if let Ok(id) = std::env::var("XDG_SESSION_ID") {
        if !id.is_empty() {
            return id;
        }
    }

    // Synchronous fallback at startup
    if let Ok(output) = std::process::Command::new("loginctl")
        .args(["--no-legend", "--no-pager", "list-sessions"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let uid = unsafe { libc::getuid() };
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Format: SESSION_ID UID USER SEAT ...
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
