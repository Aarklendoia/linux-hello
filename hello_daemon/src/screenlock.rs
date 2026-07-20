//! Screen lock monitoring and automatic facial authentication
//!
//! Polls org.freedesktop.ScreenSaver.GetActive() every 500 ms to detect
//! locking. When the screen locks, triggers facial auth without the user
//! having to press Enter. If the face is recognized, unlocks via loginctl.
//!
//! Polling is used instead of subscribing to the ActiveChanged signal —
//! originally to avoid a futures-util version conflict pinned by sqlx; sqlx
//! is no longer a dependency, so that constraint is gone, but switching to
//! signal-based detection is a separate change, not done here.
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

    // Deliberately no pre-flight `get_active()` check here: at login time
    // org.freedesktop.ScreenSaver may not be registered yet (kwin_wayland
    // registers it slightly after the session bus becomes available), and a
    // failure at this point used to abort this whole function, meaning
    // `screenlock_loop` — the only code that ever listens on
    // `retry_notify` — never started for the rest of the daemon's lifetime.
    // The loop below already retries transient GetActive() errors every 2s,
    // so let it own that recovery instead of gating startup on it.
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

/// Path of the file holding the shared-secret token every request to this
/// server must present (see `start_screenlock_control_server`'s doc comment
/// for why one was added). Same directory/reasoning as
/// `control_port_file_path`.
fn control_token_file_path() -> Option<String> {
    std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(|dir| format!("{}/hello-daemon-screenlock-ctrl.token", dir))
}

/// Generates a random 64-hex-char token, the same way
/// `crate::preview::generate_mjpeg_token`/`linux_hello_config`'s control
/// server do (reads 32 bytes directly from /dev/urandom via `read_exact`,
/// not `fs::read` — the latter would block forever on a character device
/// that never returns EOF).
fn generate_control_token() -> String {
    use std::io::Read;
    let mut buf = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .expect("Unable to read /dev/urandom for the screenlock control server token");
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Compares two strings in time that doesn't depend on *where* they first
/// differ (see `crate::preview::constant_time_eq`, which this mirrors — the
/// token's length isn't secret, always 64 hex chars by construction, so the
/// length check may still return early).
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Extracts the `X-Linux-Hello-Token:` header's value from a raw HTTP
/// request (same hand-rolled parsing style used elsewhere in this
/// project — see `linux_hello_config::main::extract_token_header`).
fn extract_token_header(req: &str) -> Option<&str> {
    req.lines().find_map(|line| {
        line.to_ascii_lowercase()
            .starts_with("x-linux-hello-token:")
            .then(|| line["x-linux-hello-token:".len()..].trim())
    })
}

/// Start the screenlock status/retry control server
/// (`GET /status`, `POST /retry`) on loopback, and write its port to
/// `control_port_file_path()`.
///
/// Gated on a shared-secret token (see `control_token_file_path`): loopback
/// TCP has no per-user ACL of its own, and `POST /retry` is a real
/// state-changing action (forces a fresh camera capture/face-match attempt
/// against whichever session this daemon instance is watching) — without a
/// token, any other local process/user could trigger unwanted camera
/// activation against someone else's locked session, or poll `/status` to
/// learn unlock timing/enrollment state as a presence side-channel.
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

    let token = generate_control_token();
    match control_token_file_path() {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, &token) {
                warn!(
                    "Could not write screenlock control token file {}: {} \
                     (status/retry UI in the lock screen won't authenticate)",
                    path, e
                );
            } else {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
                }
            }
        }
        None => warn!(
            "XDG_RUNTIME_DIR not set — can't write the screenlock control token file \
             (status/retry UI in the lock screen won't authenticate)"
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
                    let token = token.clone();
                    tokio::spawn(async move {
                        handle_control_conn(stream, status, retry_notify, &token).await;
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
    expected_token: &str,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = [0u8; 1024];
    let n = match stream.read(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let request = String::from_utf8_lossy(&buf[..n]);
    let request_line = request.lines().next().unwrap_or("");

    let token_ok = extract_token_header(&request)
        .map(|t| constant_time_eq(t, expected_token))
        .unwrap_or(false);

    let (status_code, body) = if !token_ok {
        ("403 Forbidden", String::new())
    } else if request_line.starts_with("POST /retry") {
        retry_notify.notify_one();
        ("200 OK", r#"{"ok":true}"#.to_string())
    } else if request_line.starts_with("GET /status") {
        let s = status.lock().unwrap();
        (
            "200 OK",
            format!(
                r#"{{"state":"{}","message":{}}}"#,
                s.state.as_str(),
                serde_json::to_string(&s.message).unwrap_or_else(|_| "\"\"".to_string())
            ),
        )
    } else {
        ("404 Not Found", r#"{"error":"not found"}"#.to_string())
    };

    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_code,
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

    #[test]
    fn test_extract_token_header_finds_value_case_insensitively() {
        let req = "GET /status HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Linux-Hello-Token: abc123\r\n\r\n";
        assert_eq!(extract_token_header(req), Some("abc123"));
        let req_lower = "GET /status HTTP/1.1\r\nx-linux-hello-token: abc123\r\n\r\n";
        assert_eq!(extract_token_header(req_lower), Some("abc123"));
    }

    #[test]
    fn test_extract_token_header_none_when_missing() {
        let req = "GET /status HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(extract_token_header(req), None);
    }

    #[test]
    fn test_constant_time_eq_matches_regular_equality() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc123", "abc12"));
    }

    #[test]
    fn test_generate_control_token_is_64_lowercase_hex_chars_and_varies() {
        let a = generate_control_token();
        let b = generate_control_token();
        assert_eq!(a.len(), 64);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_ne!(a, b);
    }

    #[test]
    fn control_port_and_token_file_paths_are_derived_from_xdg_runtime_dir() {
        // Read-only check against whatever XDG_RUNTIME_DIR happens to be in
        // this process — never mutated here, so safe under concurrent test
        // execution (unlike env vars this file/crate would need to change).
        match std::env::var("XDG_RUNTIME_DIR") {
            Ok(dir) => {
                assert_eq!(
                    control_port_file_path(),
                    Some(format!("{dir}/hello-daemon-screenlock-ctrl.port"))
                );
                assert_eq!(
                    control_token_file_path(),
                    Some(format!("{dir}/hello-daemon-screenlock-ctrl.token"))
                );
            }
            Err(_) => {
                assert_eq!(control_port_file_path(), None);
                assert_eq!(control_token_file_path(), None);
            }
        }
    }
}
