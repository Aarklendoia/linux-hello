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

use crate::dbus_interface::{VerifyRequest, VerifyResult};
use crate::storage::FaceStorage;
use crate::verify_with_storage;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tracing::{debug, error, info, warn};

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

    // Clean up the old socket (previous crash or update). A failure here
    // (e.g. the socket directory isn't in this unit's ReadWritePaths under
    // ProtectSystem=strict) would otherwise surface only as a confusing
    // "Address already in use" from bind() below, with no indication why.
    if let Err(e) = fs::remove_file(&socket_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!("Could not remove stale socket {}: {}", socket_path, e);
        }
    }

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

// ============================================================================
// System-wide listener (SDDM login screen)
// ============================================================================
//
// Unlike the per-user socket above (created by that user's own running
// daemon, reachable only once they're already logged in), this listener is
// started once at boot by the `hello-daemon-system` binary and never tied to
// any particular user. It backs `context=sddm`: the PAM module connects here
// instead of `/run/hello-pam/<uid>.socket` when authenticating at the login
// screen, before that user has any session (see docs/PAM_MODULE.md).
//
// Security model, deliberately tighter than the per-user socket:
// - Fixed path, mode 0600 (root-owned) rather than 0666 — only root ever
//   legitimately connects (confirmed on a live system: SDDM's `sddm-helper`,
//   which runs the actual PAM stack for /etc/pam.d/sddm, runs as root).
// - Peer credential is checked immediately after accept(), before any read —
//   this socket is reachable before authentication, so an unauthorized local
//   process must not be able to tie up a connection/fd at all.
// - Verify-only. There is no RegisterFace/DeleteFace/ListFaces here:
//   enrollment must always happen through a user's own per-user session
//   daemon, writing to their own home directory — never through this
//   listener, which only ever reads.

/// Fixed socket path for the SDDM/login-screen listener (as opposed to the
/// per-uid paths used by [`start_pam_helper`]). Overridable via
/// `LINUX_HELLO_SYSTEM_SOCKET_PATH` for testing against a scratch location
/// instead of the real, root-only `/run/hello-pam/` — unset in production.
pub fn system_socket_path() -> String {
    std::env::var("LINUX_HELLO_SYSTEM_SOCKET_PATH")
        .unwrap_or_else(|_| "/run/hello-pam/system.socket".to_string())
}

/// Resolve a UID's home directory by parsing `/etc/passwd` directly — same
/// approach already used above for `polkitd`, and in
/// `linux-hello-pam-autoconfigure` for user enumeration. No `getent`/NSS, so
/// systemd-homed-only accounts (not real `/etc/passwd` lines) won't resolve;
/// a documented limitation, not a bug.
fn resolve_home_dir(uid: u32) -> Option<std::path::PathBuf> {
    let content = std::fs::read_to_string("/etc/passwd").ok()?;
    for line in content.lines() {
        let mut fields = line.split(':');
        fields.next()?; // name
        fields.next()?; // password placeholder
        let uid_field = fields.next()?;
        if uid_field.parse::<u32>().ok()? != uid {
            continue;
        }
        fields.next()?; // gid
        fields.next()?; // gecos
        let home = fields.next()?;
        return Some(std::path::PathBuf::from(home));
    }
    None
}

/// Start the system-wide PAM socket listener for the SDDM context.
///
/// `camera`/`matcher` are long-lived and shared across requests (built once
/// by `hello-daemon-system`'s `main()`); `storage` is resolved fresh per
/// request from the target user's own home directory, via
/// [`FaceStorage::open_read_only`] — never the side-effecting `FaceStorage::new`.
pub async fn start_system_pam_helper(
    camera: Arc<crate::camera::CameraManager>,
    matcher: Arc<crate::matcher::FaceMatcher>,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = system_socket_path();
    let _ = fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path)?;
    info!("PAM system helper listening on {}", socket_path);

    // 0600: unlike the per-user socket, only root ever legitimately connects
    // here, so the filesystem permission itself closes off the unauthorized-
    // connection surface — the peer_cred check below is defense in depth,
    // not the only gate.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))?;
    }

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    #[cfg(unix)]
                    let peer_uid: Option<u32> = stream.peer_cred().ok().map(|c| c.uid());
                    #[cfg(not(unix))]
                    let peer_uid: Option<u32> = None;

                    if peer_uid != Some(0) {
                        debug!(
                            "PAM system helper: rejecting connection from non-root peer {:?}",
                            peer_uid
                        );
                        // Dropped without reading or responding — an
                        // unauthorized peer gets nothing, not even an error.
                        continue;
                    }

                    let camera = camera.clone();
                    let matcher = matcher.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_system_pam_request(stream, camera, matcher).await {
                            error!("PAM system helper error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("PAM system helper accept error: {}", e);
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Resolve the `PamHelperResponse` for a system-listener request: unknown
/// user, no enrollment, or an actual camera capture + match attempt.
///
/// Split out from [`handle_system_pam_request`] so the branch that fires
/// near-instantly (no home dir, no enrollment) can be exercised directly in
/// tests without going through the root-only socket.
async fn compute_verify_response(
    camera: &crate::camera::CameraManager,
    matcher: Arc<crate::matcher::FaceMatcher>,
    req: &PamHelperRequest,
) -> PamHelperResponse {
    match resolve_home_dir(req.user_id) {
        None => {
            debug!(
                "PAM system helper: no home directory for uid={}",
                req.user_id
            );
            PamHelperResponse::Failure {
                reason: "Unknown user".to_string(),
            }
        }
        Some(home) => {
            let base_path = home.join(".local/share/linux-hello");
            match FaceStorage::open_read_only(&base_path) {
                Ok(None) => PamHelperResponse::Failure {
                    reason: "No enrollment".to_string(),
                },
                Ok(Some(storage)) => {
                    let verify_req = VerifyRequest {
                        user_id: req.user_id,
                        context: req.context.clone(),
                        timeout_ms: req.timeout_ms,
                    };
                    let timeout = std::time::Duration::from_millis(verify_req.timeout_ms + 1000);
                    let result = tokio::time::timeout(
                        timeout,
                        verify_with_storage(&storage, camera, matcher, &verify_req),
                    )
                    .await;

                    match result {
                        Ok(Ok(VerifyResult::Success {
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
                    }
                }
                Err(e) => PamHelperResponse::Failure {
                    reason: e.to_string(),
                },
            }
        }
    }
}

/// Pad a non-`Success` response so it never returns before `req.timeout_ms`
/// has elapsed since `start`.
///
/// Mitigates the timing side-channel documented in
/// docs/PAM_MODULE.md#pam-configuration: at the SDDM greeter, "unknown
/// user" and "user exists but never enrolled" used to return in
/// microseconds, while "enrolled, camera ran" took roughly 1-5s — letting
/// someone at the greeter (or scripting repeated attempts) infer which
/// local accounts have enrolled a face just from response latency. Flooring
/// every failure to the caller's own configured `timeout_ms` (the same
/// value already used to bound the real capture attempt) closes that gap
/// without slowing down the one response worth keeping fast: a genuine
/// `Success` reveals nothing an attacker didn't already know (they *are*
/// the enrolled user), so it's left unpadded.
async fn respond_with_floor(
    start: std::time::Instant,
    req: &PamHelperRequest,
    response: PamHelperResponse,
) -> PamHelperResponse {
    if !matches!(response, PamHelperResponse::Success { .. }) {
        let floor = std::time::Duration::from_millis(req.timeout_ms);
        let elapsed = start.elapsed();
        if elapsed < floor {
            tokio::time::sleep(floor - elapsed).await;
        }
    }
    response
}

/// Process an incoming connection on the system listener.
async fn handle_system_pam_request(
    mut stream: tokio::net::UnixStream,
    camera: Arc<crate::camera::CameraManager>,
    matcher: Arc<crate::matcher::FaceMatcher>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let start = std::time::Instant::now();

    // Bounded read: defense in depth. This socket is reachable before any
    // authentication happens, so a stalled/malicious peer must not be able
    // to tie up a connection indefinitely.
    const MAX_REQUEST_BYTES: usize = 4096;
    const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

    let mut buf = Vec::new();
    let mut chunk = [0u8; 512];
    let read_result = tokio::time::timeout(READ_TIMEOUT, async {
        loop {
            let n = stream.read(&mut chunk).await?;
            if n == 0 || buf.len() > MAX_REQUEST_BYTES {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        Ok::<(), std::io::Error>(())
    })
    .await;

    if read_result.is_err() || buf.is_empty() || buf.len() > MAX_REQUEST_BYTES {
        return Ok(());
    }

    let request_json = String::from_utf8(buf)?;
    debug!("PAM system helper received: {}", request_json);
    let req: PamHelperRequest = serde_json::from_str(&request_json)?;

    let response = compute_verify_response(&camera, matcher, &req).await;
    let response = respond_with_floor(start, &req, response).await;

    let response_json = serde_json::to_string(&response)?;
    stream.write_all(response_json.as_bytes()).await?;
    stream.shutdown().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_home_dir_current_user() {
        // Read-only against the real /etc/passwd; resolves the uid this
        // test process is actually running as (safe — no writes).
        let uid = unsafe { libc::getuid() };
        let home = resolve_home_dir(uid).expect("current uid should resolve from /etc/passwd");
        assert!(!home.as_os_str().is_empty());
    }

    #[test]
    fn test_resolve_home_dir_nonexistent_uid() {
        // A UID astronomically unlikely to exist on any real system.
        assert!(resolve_home_dir(4_294_967_000).is_none());
    }

    #[test]
    fn test_system_socket_path_override() {
        // SAFETY: no other test in this binary reads this env var.
        unsafe {
            std::env::set_var(
                "LINUX_HELLO_SYSTEM_SOCKET_PATH",
                "/tmp/test-override.socket",
            );
        }
        assert_eq!(system_socket_path(), "/tmp/test-override.socket");
        unsafe {
            std::env::remove_var("LINUX_HELLO_SYSTEM_SOCKET_PATH");
        }
    }

    fn unknown_user_req(timeout_ms: u64) -> PamHelperRequest {
        PamHelperRequest {
            // Same "astronomically unlikely to exist" uid used above —
            // resolve_home_dir returns None, so this hits the fast
            // "Unknown user" branch.
            user_id: 4_294_967_000,
            context: "sddm".to_string(),
            timeout_ms,
        }
    }

    #[tokio::test]
    async fn test_compute_verify_response_unknown_user_is_a_fast_failure() {
        let camera = crate::camera::CameraManager::new(1000);
        let matcher = Arc::new(crate::matcher::FaceMatcher::new(0.6));
        let req = unknown_user_req(5000);

        let response = compute_verify_response(&camera, matcher, &req).await;

        assert!(matches!(
            response,
            PamHelperResponse::Failure { reason } if reason == "Unknown user"
        ));
    }

    /// Regression test for the timing side-channel described in
    /// docs/PAM_MODULE.md: without `respond_with_floor`, this branch returns
    /// in microseconds — a local attacker at the greeter could distinguish
    /// it from the ~1-5s an actual camera capture takes. Padding it to the
    /// caller-supplied `timeout_ms` closes that gap.
    #[tokio::test]
    async fn test_respond_with_floor_pads_a_fast_failure_up_to_timeout_ms() {
        let camera = crate::camera::CameraManager::new(1000);
        let matcher = Arc::new(crate::matcher::FaceMatcher::new(0.6));
        let req = unknown_user_req(150);

        let start = std::time::Instant::now();
        let response = compute_verify_response(&camera, matcher, &req).await;
        let response = respond_with_floor(start, &req, response).await;

        assert!(matches!(response, PamHelperResponse::Failure { .. }));
        assert!(
            start.elapsed() >= std::time::Duration::from_millis(150),
            "elapsed={:?} should be padded up to timeout_ms",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn test_respond_with_floor_does_not_pad_success() {
        let req = unknown_user_req(5000); // timeout_ms large enough that padding would be obvious
        let start = std::time::Instant::now();

        let response = respond_with_floor(
            start,
            &req,
            PamHelperResponse::Success {
                face_id: "face_1".to_string(),
                similarity_score: 0.9,
            },
        )
        .await;

        assert!(matches!(response, PamHelperResponse::Success { .. }));
        assert!(
            start.elapsed() < std::time::Duration::from_millis(500),
            "a genuine success must not be held back by the floor"
        );
    }

    #[tokio::test]
    async fn test_respond_with_floor_adds_no_extra_delay_once_past_the_floor() {
        let req = unknown_user_req(30);
        // Backdate `start` so the floor has already elapsed by the time we
        // call respond_with_floor — it must not sleep an extra `timeout_ms`
        // on top.
        let start = std::time::Instant::now() - std::time::Duration::from_millis(200);

        let before = std::time::Instant::now();
        let _ = respond_with_floor(
            start,
            &req,
            PamHelperResponse::Failure {
                reason: "Timeout".to_string(),
            },
        )
        .await;

        assert!(
            before.elapsed() < std::time::Duration::from_millis(100),
            "must not add extra delay once the floor has already elapsed"
        );
    }
}
