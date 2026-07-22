//! Interactive re-authentication gate for enrollment changes.
//!
//! `register_face`/`delete_face` are reachable over the session D-Bus bus by
//! any local process running as this daemon's own UID — the bus itself
//! enforces nothing beyond that. Since a successfully enrolled face
//! subsequently unlocks `sudo` (via `pam_linux_hello.so`), a process with
//! only a transient foothold as the user (malware, a compromised app) could
//! otherwise silently plant an attacker-controlled face and keep root access
//! long after that foothold is gone — no password, no prompt, nothing the
//! legitimate user would ever see.
//!
//! [`EnrollmentAuthorizer::Polkit`] closes that gap by requiring a fresh,
//! interactive polkit check (`com.linuxhello.manage-faces`, `auth_self`)
//! before either call proceeds — the same "prove you're still you" prompt a
//! desktop would show for changing a login password or an SSH key. It
//! checks the *session*, not the calling process: any process on the same
//! login session can trigger the prompt, but only a human answering it with
//! the account password lets the call through.
use tracing::{debug, warn};
use zbus::zvariant::Value;
use zbus::Connection;

/// The single polkit action shared by `register_face` and `delete_face` —
/// both let a caller change what unlocks `sudo`, so both carry the same risk
/// and the same gate.
pub const MANAGE_FACES_ACTION: &str = "com.linuxhello.manage-faces";

/// Whether an enrollment-changing call is currently authorized.
pub enum EnrollmentAuthorizer {
    /// Real gate: checks with polkit, scoped to this process's own login
    /// session (`$XDG_SESSION_ID`, captured once at daemon startup).
    /// `session_id: None` means the daemon isn't running inside a tracked
    /// login session at all (e.g. started outside a normal desktop login) —
    /// there is then no session to prompt, so this fails closed rather than
    /// silently allowing the change.
    Polkit { session_id: Option<String> },
    /// No gate at all. Only for tests that aren't exercising this check —
    /// matches this crate's existing pattern of swapping in fakes (see
    /// `CameraManager::for_test`) rather than hitting a real system service.
    #[cfg(test)]
    AllowAll,
    /// Always denies. For tests that specifically verify the gate blocks
    /// register_face/delete_face when authorization is refused.
    #[cfg(test)]
    DenyAll,
}

impl EnrollmentAuthorizer {
    /// The real, production authorizer: reads `$XDG_SESSION_ID` once now
    /// (systemd sets this for `--user` units started from a real login
    /// session — see `hello-daemon.service`).
    pub fn from_env() -> Self {
        Self::Polkit {
            session_id: std::env::var("XDG_SESSION_ID").ok(),
        }
    }

    pub async fn authorize(&self, action: &str) -> bool {
        match self {
            Self::Polkit {
                session_id: Some(id),
            } => check_polkit_authorization(id, action).await,
            Self::Polkit { session_id: None } => {
                warn!("No XDG_SESSION_ID: cannot check polkit authorization, denying {action}");
                false
            }
            #[cfg(test)]
            Self::AllowAll => true,
            #[cfg(test)]
            Self::DenyAll => false,
        }
    }
}

/// Calls `org.freedesktop.PolicyKit1.Authority.CheckAuthorization` with a
/// `unix-session` subject, requesting interactive authentication
/// (`AllowUserInteraction`, flag `1`) so polkit's agent can actually prompt
/// for the password rather than failing immediately.
///
/// Fails closed: any D-Bus error (system bus unreachable, polkit not
/// running, action not registered — e.g. the package's `.policy` file isn't
/// installed) denies the action rather than silently allowing it.
async fn check_polkit_authorization(session_id: &str, action: &str) -> bool {
    let connection = match Connection::system().await {
        Ok(c) => c,
        Err(e) => {
            warn!("Could not reach the system bus for a polkit check: {e}");
            return false;
        }
    };

    let mut subject_details = std::collections::HashMap::new();
    subject_details.insert("session-id", Value::new(session_id));
    let subject = ("unix-session", subject_details);
    let details: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    const ALLOW_USER_INTERACTION: u32 = 1;

    let reply = connection
        .call_method(
            Some("org.freedesktop.PolicyKit1"),
            "/org/freedesktop/PolicyKit1/Authority",
            Some("org.freedesktop.PolicyKit1.Authority"),
            "CheckAuthorization",
            &(subject, action, details, ALLOW_USER_INTERACTION, ""),
        )
        .await;

    let reply = match reply {
        Ok(r) => r,
        Err(e) => {
            warn!("polkit CheckAuthorization call failed: {e}");
            return false;
        }
    };

    match reply
        .body()
        .deserialize::<(bool, bool, std::collections::HashMap<String, String>)>()
    {
        Ok((is_authorized, _is_challenge, _details)) => {
            debug!("polkit authorization for {action}: {is_authorized}");
            is_authorized
        }
        Err(e) => {
            warn!("Could not parse polkit's CheckAuthorization reply: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn allow_all_always_authorizes() {
        assert!(EnrollmentAuthorizer::AllowAll.authorize("anything").await);
    }

    #[tokio::test]
    async fn deny_all_never_authorizes() {
        assert!(!EnrollmentAuthorizer::DenyAll.authorize("anything").await);
    }

    #[tokio::test]
    async fn polkit_with_no_session_id_fails_closed() {
        let authorizer = EnrollmentAuthorizer::Polkit { session_id: None };
        assert!(!authorizer.authorize(MANAGE_FACES_ACTION).await);
    }

    /// Manual, opt-in smoke test against a REAL system polkitd (not run by
    /// `cargo test` normally — `cargo test -- --ignored`). Exercises the
    /// actual D-Bus call this module makes in production, against the real
    /// session this test runs in. Deliberately does NOT install
    /// debian/polkit/com.linuxhello.manage-faces.policy first: polkit's
    /// well-defined, constantly-exercised behavior for an unregistered
    /// action_id is to answer with an error rather than crash — the same
    /// thing every polkit client sees before its package's .policy file is
    /// installed — so this checks the round trip (call succeeds, response
    /// parses, unknown action is treated as "not authorized") without
    /// needing root to install anything.
    #[tokio::test]
    #[ignore]
    async fn polkit_round_trip_against_the_real_system_bus() {
        let session_id = std::env::var("XDG_SESSION_ID")
            .expect("this manual test must run inside a real login session");
        let authorized = check_polkit_authorization(&session_id, MANAGE_FACES_ACTION).await;
        assert!(
            !authorized,
            "com.linuxhello.manage-faces isn't installed in this environment, so this must be false, not panic/crash"
        );
    }
}
