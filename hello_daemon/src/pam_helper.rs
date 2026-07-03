//! PAM Helper daemon to work around D-Bus isolation
//!
//! Problem: The PAM module runs as root and cannot access the user's D-Bus.
//! Solution: A helper daemon that runs as the user and acts as a bridge.
//!
//! Communication: Unix socket at `/tmp/hello-pam-UID.socket`
//!
//! IMPORTANT: uses tokio::net::UnixListener (async) because std::net::UnixListener
//! becomes non-blocking when used in a tokio context, which causes
//! EAGAIN on read() on the PAM side.

use crate::dbus_interface::VerifyRequest;
use serde::{Deserialize, Serialize};
use std::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tracing::{debug, error, info};

/// PAM request via socket
#[derive(Debug, Serialize, Deserialize)]
pub struct PamHelperRequest {
    pub user_id: u32,
    pub context: String,
    pub timeout_ms: u64,
}

/// Helper response
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

/// Start the PAM socket listener (async tokio)
pub async fn start_pam_helper(
    uid: u32,
    daemon: std::sync::Arc<tokio::sync::RwLock<crate::FaceAuthDaemon>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // /run/hello-pam/ is created by systemd-tmpfiles (mode 1777, sticky):
    //   - the user daemon (uid=1000) can create its socket there
    //   - polkitd (PrivateTmp=yes) can access it: /run is not isolated by PrivateTmp
    //   - sudo-rs (uid=65534) and root can connect to it
    // Security relies on peer_cred in handle_pam_request, not on the path.
    let socket_path = format!("/run/hello-pam/{}.socket", uid);

    // Clean up the old socket (previous crash or update)
    let _ = fs::remove_file(&socket_path);

    // tokio::net::UnixListener: fully async, no EAGAIN issue
    let listener = UnixListener::bind(&socket_path)?;
    info!("PAM Helper listening on {}", socket_path);

    // 0o666: accessible to all processes (polkitd, sudo-rs, etc.).
    // Security relies on peer_cred validation in handle_pam_request:
    // only root, the target user and nobody (sudo-rs) can get a response.
    // A third-party process can connect but will be rejected with "Unauthorized".
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o666))?;
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

/// Send a failure response and close the stream.
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

/// Process an incoming PAM connection
async fn handle_pam_request(
    mut stream: tokio::net::UnixStream,
    daemon: std::sync::Arc<tokio::sync::RwLock<crate::FaceAuthDaemon>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get the UID of the connecting process before reading anything.
    // This prevents an attacker from forging a user_id different from their own.
    #[cfg(unix)]
    let peer_uid: Option<u32> = stream.peer_cred().ok().map(|c| c.uid());
    #[cfg(not(unix))]
    let peer_uid: Option<u32> = None;

    // Read everything the client sends (it does shutdown(Write) afterward)
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;

    if buf.is_empty() {
        return Ok(());
    }

    let request_json = String::from_utf8(buf)?;
    debug!("PAM Helper received: {}", request_json);

    let req: PamHelperRequest = serde_json::from_str(&request_json)?;

    // Validate the peer:
    // - uid=0      (root)    : classic sudo
    // - uid=user   (edtech)  : direct CLI/GUI call
    // - uid=65534  (nobody)  : sudo-rs sandbox
    // - uid=polkitd          : pkexec and polkit graphical dialogs
    const NOBODY: u32 = 65534;
    let polkitd_uid: u32 = std::fs::read_to_string("/etc/passwd")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("polkitd:"))
                .and_then(|l| l.split(':').nth(2))
                .and_then(|u| u.parse().ok())
        })
        .unwrap_or(987);
    if let Some(uid) = peer_uid {
        if uid != 0 && uid != req.user_id && uid != NOBODY && uid != polkitd_uid {
            error!(
                "PAM helper: connection refused — peer uid={} requested user_id={}",
                uid, req.user_id
            );
            return reject(
                stream,
                "Unauthorized: peer uid does not match requested user_id",
            )
            .await;
        }
    }

    let verify_req = VerifyRequest {
        user_id: req.user_id,
        context: req.context,
        timeout_ms: req.timeout_ms,
    };

    // Call the daemon (timeout = requested timeout + 1s margin)
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
