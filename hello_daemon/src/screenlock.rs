//! Screen lock monitoring and automatic facial authentication
//!
//! Polls org.freedesktop.ScreenSaver.GetActive() every 500 ms to detect
//! locking. When the screen locks, triggers facial auth without the user
//! having to press Enter. If the face is recognized, unlocks via loginctl.
//!
//! Polling is used instead of subscribing to the ActiveChanged signal to avoid
//! a conflicting dependency on futures-util (pinned to 0.3.32 by sqlx).
//!
//! A face-recognition attempt only ever fired once, at the lock transition,
//! with no way to retry if the user didn't come back within its timeout —
//! making the feature useless the moment you walk away and return later.
//! This module now also exposes a small local control server (`GET /status`,
//! `POST /retry`) so `qml/lockscreen/MainBlock.qml` can show live status and
//! let the user retry on demand (e.g. when they notice the screen and it's
//! past the original attempt's window) or fall back to the password field.

use crate::dbus_interface::{VerifyRequest, VerifyResult};
use crate::FaceAuthDaemon;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Notify, RwLock};
use tracing::{error, info, warn};
use zbus::proxy;

/// Fixed port of the screenlock status/retry control server (loopback only).
/// Distinct from `preview::MJPEG_PORT` (17823).
pub const SCREENLOCK_CTRL_PORT: u16 = 17824;

/// Minimum delay between two face-recognition attempts, whether triggered by
/// a lock transition or a retry request — avoids hammering the camera if the
/// lock screen fires several retries in quick succession.
const RETRY_COOLDOWN: Duration = Duration::from_secs(4);

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

/// Live state of the screenlock face-recognition feature, polled by
/// `MainBlock.qml` via the control server started by
/// `start_screenlock_control_server`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenlockState {
    /// No attempt in progress (screen unlocked, or between attempts).
    Idle,
    /// A capture/match is currently running.
    Recognizing,
    /// The last attempt recognized the face and unlocked the session.
    Success,
    /// The last attempt did not recognize the face, or errored out.
    Failed,
}

impl ScreenlockState {
    fn as_str(self) -> &'static str {
        match self {
            ScreenlockState::Idle => "idle",
            ScreenlockState::Recognizing => "recognizing",
            ScreenlockState::Success => "success",
            ScreenlockState::Failed => "failed",
        }
    }
}

#[derive(Debug)]
pub struct ScreenlockStatus {
    pub state: ScreenlockState,
    /// Human-readable detail (e.g. a failure reason) — English/internal, not
    /// localized; `MainBlock.qml` displays its own localized text keyed off
    /// `state` and only surfaces this for debugging.
    pub message: String,
    last_attempt_started: Option<Instant>,
}

impl Default for ScreenlockStatus {
    fn default() -> Self {
        Self {
            state: ScreenlockState::Idle,
            message: String::new(),
            last_attempt_started: None,
        }
    }
}

pub type SharedScreenlockStatus = Arc<Mutex<ScreenlockStatus>>;

/// Start monitoring the screen lock.
///
/// Returns immediately; the monitoring loop runs in a dedicated tokio task.
pub async fn start_screenlock_watcher(
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    user_id: u32,
    status: SharedScreenlockStatus,
    retry_notify: Arc<Notify>,
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
        screenlock_loop(proxy, daemon, user_id, session_id, status, retry_notify).await;
    });

    Ok(())
}

/// Main loop: polling every 500 ms, detecting lock/unlock transitions, and
/// reacting to on-demand retry requests from the control server.
async fn screenlock_loop(
    proxy: ScreenSaverProxy<'_>,
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    user_id: u32,
    session_id: String,
    status: SharedScreenlockStatus,
    retry_notify: Arc<Notify>,
) {
    let mut was_active = false;

    info!("Screen lock monitoring active (polling 500 ms)");

    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
            _ = retry_notify.notified() => {
                if was_active {
                    info!("Retry requested → attempting facial auth");
                    maybe_spawn_attempt(&daemon, user_id, &session_id, &status);
                }
                continue;
            }
        }

        let is_active = match proxy.get_active().await {
            Ok(v) => v,
            Err(e) => {
                warn!("ScreenSaver.GetActive() error: {} — retrying in 2s", e);
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        if is_active && !was_active {
            // Transition → locked
            info!("Screen lock detected → launching automatic facial auth");
            was_active = true;
            maybe_spawn_attempt(&daemon, user_id, &session_id, &status);
        } else if !is_active && was_active {
            // Transition → unlocked (via face or password)
            info!("Screen unlocked");
            was_active = false;
            *status.lock().unwrap() = ScreenlockStatus::default();
        }
    }
}

/// Claims the right to start a new attempt: `true` and marks the status
/// `Recognizing` if none is already running and the last one didn't start
/// too recently (`RETRY_COOLDOWN`); otherwise leaves the status untouched
/// and returns `false`. Split out from `maybe_spawn_attempt` so this
/// decision can be unit-tested without a real `FaceAuthDaemon`/tokio spawn.
fn try_claim_attempt(status: &SharedScreenlockStatus) -> bool {
    let mut s = status.lock().unwrap();
    if s.state == ScreenlockState::Recognizing {
        return false;
    }
    if let Some(last) = s.last_attempt_started {
        if last.elapsed() < RETRY_COOLDOWN {
            return false;
        }
    }
    s.state = ScreenlockState::Recognizing;
    s.message.clear();
    s.last_attempt_started = Some(Instant::now());
    true
}

/// Spawn a face-recognition attempt unless one is already running or the
/// last one started too recently (`RETRY_COOLDOWN`) — shared by both the
/// lock-transition path and the on-demand retry path so neither can launch
/// overlapping camera captures.
fn maybe_spawn_attempt(
    daemon: &Arc<RwLock<FaceAuthDaemon>>,
    user_id: u32,
    session_id: &str,
    status: &SharedScreenlockStatus,
) {
    if !try_claim_attempt(status) {
        return;
    }

    let daemon = daemon.clone();
    let session_id = session_id.to_string();
    let status = status.clone();
    tokio::spawn(async move {
        try_face_unlock(daemon, user_id, &session_id, status).await;
    });
}

/// Attempt facial authentication and unlock on success.
async fn try_face_unlock(
    daemon: Arc<RwLock<FaceAuthDaemon>>,
    user_id: u32,
    session_id: &str,
    status: SharedScreenlockStatus,
) {
    // Give the lock screen time to fully render.
    tokio::time::sleep(Duration::from_millis(1200)).await;

    info!(
        "Automatic facial auth for uid={} (context=screenlock, timeout=30s)",
        user_id
    );

    let result = {
        let d = daemon.read().await;
        d.verify(VerifyRequest {
            user_id,
            context: "screenlock".to_string(),
            timeout_ms: 30000,
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
            match unlock_session(session_id).await {
                Ok(()) => {
                    let mut s = status.lock().unwrap();
                    s.state = ScreenlockState::Success;
                    s.message = format!("face_id={} score={:.3}", face_id, similarity_score);
                }
                Err(e) => {
                    error!("loginctl unlock failed: {}", e);
                    let mut s = status.lock().unwrap();
                    s.state = ScreenlockState::Failed;
                    s.message = format!("unlock failed: {}", e);
                }
            }
        }
        Ok(VerifyResult::NoEnrollment) => {
            info!("No face registered for uid={} — no automatic auth", user_id);
            let mut s = status.lock().unwrap();
            s.state = ScreenlockState::Idle;
            s.message = "no enrolled face".to_string();
        }
        Ok(other) => {
            info!(
                "Facial auth inconclusive ({}): the user can enter their password",
                other
            );
            let mut s = status.lock().unwrap();
            s.state = ScreenlockState::Failed;
            s.message = other.to_string();
        }
        Err(e) => {
            warn!(
                "Facial auth error: {} — the user can enter their password",
                e
            );
            let mut s = status.lock().unwrap();
            s.state = ScreenlockState::Failed;
            s.message = e.to_string();
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

/// Path of the file `MainBlock.qml` reads (via a `Plasma5Support.DataSource`
/// shell command — QML's own `XMLHttpRequest` is blocked by
/// `kscreenlocker_greet`'s QML engine, confirmed empirically) to learn which
/// port the control server below is listening on.
///
/// `$XDG_RUNTIME_DIR` (not `/tmp`, unlike the GUI's own control-server port
/// file): `/tmp` isn't in `hello-daemon.service`'s `ReadWritePaths` and,
/// under `ProtectSystem=strict`, anything not explicitly listed there fails
/// to write silently; `$XDG_RUNTIME_DIR` (`/run/user/<uid>`) is already
/// granted via the unit's existing `%t` entry.
fn control_port_file_path() -> Option<String> {
    std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(|dir| format!("{}/hello-daemon-screenlock-ctrl.port", dir))
}

/// Start the screenlock status/retry control server
/// (`GET /status`, `POST /retry`) on loopback, and write its port to
/// `control_port_file_path()`.
///
/// Same hand-rolled raw-tokio-TCP style as `crate::preview::start_mjpeg_server`
/// — no HTTP framework dependency.
pub async fn start_screenlock_control_server(
    status: SharedScreenlockStatus,
    retry_notify: Arc<Notify>,
) -> std::io::Result<()> {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", SCREENLOCK_CTRL_PORT)).await?;

    match control_port_file_path() {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, SCREENLOCK_CTRL_PORT.to_string()) {
                warn!(
                    "Could not write screenlock control port file {}: {} \
                     (status/retry UI in the lock screen won't find the server)",
                    path, e
                );
            }
        }
        None => warn!(
            "XDG_RUNTIME_DIR not set — can't write the screenlock control port file \
             (status/retry UI in the lock screen won't find the server)"
        ),
    }

    info!(
        "Screenlock control server: http://127.0.0.1:{}",
        SCREENLOCK_CTRL_PORT
    );

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let status = status.clone();
                    let retry_notify = retry_notify.clone();
                    tokio::spawn(async move {
                        handle_control_conn(stream, status, retry_notify).await;
                    });
                }
                Err(e) => error!("Screenlock control server accept error: {}", e),
            }
        }
    });

    Ok(())
}

async fn handle_control_conn(
    mut stream: tokio::net::TcpStream,
    status: SharedScreenlockStatus,
    retry_notify: Arc<Notify>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = [0u8; 1024];
    let n = match stream.read(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let request = String::from_utf8_lossy(&buf[..n]);
    let request_line = request.lines().next().unwrap_or("");

    let body = if request_line.starts_with("POST /retry") {
        retry_notify.notify_one();
        r#"{"ok":true}"#.to_string()
    } else if request_line.starts_with("GET /status") {
        let s = status.lock().unwrap();
        format!(
            r#"{{"state":"{}","message":{}}}"#,
            s.state.as_str(),
            serde_json::to_string(&s.message).unwrap_or_else(|_| "\"\"".to_string())
        )
    } else {
        r#"{"error":"not found"}"#.to_string()
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared_default() -> SharedScreenlockStatus {
        Arc::new(Mutex::new(ScreenlockStatus::default()))
    }

    #[test]
    fn test_try_claim_attempt_succeeds_when_idle() {
        let status = shared_default();
        assert!(try_claim_attempt(&status));
        assert_eq!(status.lock().unwrap().state, ScreenlockState::Recognizing);
    }

    #[test]
    fn test_try_claim_attempt_fails_while_already_recognizing() {
        let status = shared_default();
        assert!(try_claim_attempt(&status));
        // A second claim while the first is still "in flight" must not
        // launch an overlapping capture.
        assert!(!try_claim_attempt(&status));
    }

    #[test]
    fn test_try_claim_attempt_respects_cooldown_after_completion() {
        let status = shared_default();
        assert!(try_claim_attempt(&status));
        // Simulate the attempt finishing (Success/Failed), same as
        // try_face_unlock would do, but immediately — too soon for a retry.
        status.lock().unwrap().state = ScreenlockState::Failed;
        assert!(!try_claim_attempt(&status));
    }

    #[test]
    fn test_try_claim_attempt_allows_retry_after_cooldown_elapsed() {
        let status = shared_default();
        assert!(try_claim_attempt(&status));
        {
            let mut s = status.lock().unwrap();
            s.state = ScreenlockState::Failed;
            // Backdate the last attempt past the cooldown window instead of
            // sleeping in the test.
            s.last_attempt_started = Some(Instant::now() - RETRY_COOLDOWN - Duration::from_secs(1));
        }
        assert!(try_claim_attempt(&status));
    }

    #[test]
    fn test_screenlock_state_as_str() {
        assert_eq!(ScreenlockState::Idle.as_str(), "idle");
        assert_eq!(ScreenlockState::Recognizing.as_str(), "recognizing");
        assert_eq!(ScreenlockState::Success.as_str(), "success");
        assert_eq!(ScreenlockState::Failed.as_str(), "failed");
    }
}
