//! Shared security-sensitive primitives, used by more than one of this
//! crate's local servers (`preview`'s MJPEG server, `screenlock`'s
//! status/retry server) and `storage`'s on-disk face records — previously
//! each had its own copy, cross-referencing the others in doc comments
//! rather than sharing the actual code.

use std::path::Path;

/// Generates a random 64-hex-char token by reading exactly 32 bytes
/// directly from `/dev/urandom` (`read_exact`, not `fs::read` — the latter
/// would block forever on a character device that never returns EOF).
pub(crate) fn generate_token() -> String {
    use std::io::Read;
    let mut buf = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .expect("Unable to read /dev/urandom for a security token");
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Compares two strings in time that doesn't depend on *where* they first
/// differ. The tokens compared here are never secret in length (always
/// 64 hex chars by construction), so the length check may still return
/// early.
pub(crate) fn constant_time_eq(a: &str, b: &str) -> bool {
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

/// Writes `contents` to `path` at mode `0600`, owned by the current
/// process. Removes whatever is at `path` first, then creates fresh with
/// `create_new` (`O_CREAT|O_EXCL`) so the mode is applied atomically at
/// creation, rather than via a separate `set_permissions` call that would
/// leave a moment where the file exists with default/umask permissions.
pub(crate) fn write_owner_only_file(path: impl AsRef<Path>, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let path = path.as_ref();
    let _ = std::fs::remove_file(path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(contents.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_token_is_64_lowercase_hex_chars_and_varies() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), 64);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_ne!(a, b);
    }

    #[test]
    fn constant_time_eq_matches_regular_equality() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc123", "abc12"));
        assert!(!constant_time_eq("", "abc123"));
    }

    #[test]
    fn write_owner_only_file_sets_mode_0600_and_replaces_existing_content() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "hello-daemon-security-util-test-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, "stale content").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();

        write_owner_only_file(&path, "fresh token").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "fresh token");

        let _ = std::fs::remove_file(&path);
    }
}
