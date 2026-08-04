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
    /// Real gate: checks with polkit, using a `unix-process` subject built
    /// from this daemon's own PID + start time.
    ///
    /// Not the more obvious `unix-session` subject (session-id string,
    /// what this used to send): on at least one real, current-Ubuntu
    /// polkitd (127-2ubuntu1), a `CheckAuthorization` call with a
    /// `unix-session` subject reliably crashes the daemon with a glib
    /// assertion failure — reproduced with a raw `busctl` call, no
    /// linux-hello code involved — taking the in-flight request down with
    /// it as an opaque "NoReply: Message recipient disconnected", which is
    /// what actually broke every enrollment attempt (not just the
    /// GUI/camera timing issues fixed alongside this). A `unix-process`
    /// subject exercises a different, unaffected code path in polkitd and
    /// resolves to the very same session (polkit looks up the session a
    /// process belongs to), so the check keeps its original meaning — it
    /// just names that session through a live process in it (this daemon
    /// itself) instead of a session-id string.
    Polkit,
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
    pub async fn authorize(&self, action: &str) -> bool {
        match self {
            Self::Polkit => check_polkit_authorization(action).await,
            #[cfg(test)]
            Self::AllowAll => true,
            #[cfg(test)]
            Self::DenyAll => false,
        }
    }
}

/// Parses the `starttime` field (22nd, in clock ticks since boot) out of a
/// `/proc/<pid>/stat`-shaped string — split out from `own_process_start_time`
/// so the tricky parsing can be tested directly, with no real `/proc` needed.
///
/// Can't just split the whole line on whitespace from the start: field 2
/// (`comm`, the process name) is parenthesized specifically because it can
/// itself contain spaces or parentheses (documented in proc(5)), which would
/// misalign every fixed-position field after it. Finding the *last* `)`
/// instead — proc(5)'s own recommended approach — sidesteps that: everything
/// after it is fields 3 onward, unambiguously whitespace-separated.
fn parse_start_time_from_stat(stat: &str) -> Option<u64> {
    let after_comm = stat.rfind(')')?;
    let fields: Vec<&str> = stat[after_comm + 1..].split_whitespace().collect();
    // fields[0] is field 3 (state), so field 22 (starttime) is fields[22-3].
    fields.get(22 - 3)?.parse().ok()
}

/// Reads this process's own start time from `/proc/self/stat` — the second
/// piece (together with the PID) polkit needs to identify a `unix-process`
/// subject without a PID-reuse ambiguity.
fn own_process_start_time() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    parse_start_time_from_stat(&stat)
}

/// Calls `org.freedesktop.PolicyKit1.Authority.CheckAuthorization` with a
/// `unix-process` subject identifying this daemon process, requesting
/// interactive authentication (`AllowUserInteraction`, flag `1`) so
/// polkit's agent can actually prompt for the password rather than
/// failing immediately.
///
/// Fails closed: any D-Bus error (system bus unreachable, polkit not
/// running, action not registered — e.g. the package's `.policy` file isn't
/// installed) denies the action rather than silently allowing it.
async fn check_polkit_authorization(action: &str) -> bool {
    let Some(start_time) = own_process_start_time() else {
        warn!(
            "Could not read this process's own start time from /proc/self/stat, denying {action}"
        );
        return false;
    };

    let connection = match Connection::system().await {
        Ok(c) => c,
        Err(e) => {
            warn!("Could not reach the system bus for a polkit check: {e}");
            return false;
        }
    };

    let mut subject_details = std::collections::HashMap::new();
    subject_details.insert("pid", Value::new(std::process::id()));
    subject_details.insert("start-time", Value::new(start_time));
    let subject = ("unix-process", subject_details);
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

    #[test]
    fn parse_start_time_from_stat_finds_field_22_after_the_comm_field() {
        // A realistic /proc/self/stat shape: pid, (comm), then 20 more
        // whitespace-separated fields (state..starttime) — starttime
        // (field 22) is the last one here, set to a recognizable value.
        let stat =
            "12345 (hello-daemon) S 1 12345 12345 0 -1 4194560 0 0 0 0 0 0 0 0 0 0 0 0 3144151";
        assert_eq!(parse_start_time_from_stat(stat), Some(3144151));
    }

    #[test]
    fn parse_start_time_from_stat_handles_a_comm_field_containing_parens_and_spaces() {
        // proc(5): comm can itself contain spaces or parentheses — a naive
        // split on the first '(' / first ')' would misalign every field
        // after it. Same 20 trailing fields as above.
        let stat =
            "12345 (my (weird) prog) S 1 12345 12345 0 -1 4194560 0 0 0 0 0 0 0 0 0 0 0 0 3144151";
        assert_eq!(parse_start_time_from_stat(stat), Some(3144151));
    }

    #[test]
    fn parse_start_time_from_stat_none_when_too_short_or_malformed() {
        assert_eq!(parse_start_time_from_stat("12345 (hello-daemon) S 1"), None);
        assert_eq!(parse_start_time_from_stat("no parens here at all"), None);
        assert_eq!(parse_start_time_from_stat(""), None);
    }

    /// Manual, opt-in smoke test against a REAL system polkitd (not run by
    /// `cargo test` normally — `cargo test -- --ignored`). Exercises the
    /// actual D-Bus call this module makes in production. Deliberately does
    /// NOT install debian/polkit/com.linuxhello.manage-faces.policy first:
    /// polkit's well-defined, constantly-exercised behavior for an
    /// unregistered action_id is to answer with an error rather than crash
    /// — the same thing every polkit client sees before its package's
    /// .policy file is installed — so this checks the round trip (call
    /// succeeds, response parses, unknown action is treated as "not
    /// authorized") without needing root to install anything, and without
    /// crashing the real polkitd it's running against (the whole reason
    /// this module moved off `unix-session` subjects).
    #[tokio::test]
    #[ignore]
    async fn polkit_round_trip_against_the_real_system_bus() {
        let authorized = check_polkit_authorization(MANAGE_FACES_ACTION).await;
        assert!(
            !authorized,
            "com.linuxhello.manage-faces isn't installed in this environment, so this must be false, not panic/crash"
        );
    }
}
