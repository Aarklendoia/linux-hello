//! Linux Hello - Configuration GUI for KDE/Wayland
//!
//! Simple Qt6/QML launcher that:
//! - Launches the QML engine via qml6
//! - Displays the configuration interface
//! - Integrates the daemon's live video preview
//!
//! The daemon (hello_daemon) exports frames via D-Bus

use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::thread;

fn main() {
    // Determine the QML path
    let qml_path = find_qml_path();

    let uid = get_current_uid();

    // Prevent multiple simultaneous instances
    let lock_path = format!("/tmp/linux-hello-config-{}.lock", uid);
    if let Ok(content) = std::fs::read_to_string(&lock_path) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            if std::path::Path::new(&format!("/proc/{}", pid)).exists() {
                eprintln!(
                    "⚠ Linux Hello is already open (PID {}). Only one instance is allowed.",
                    pid
                );
                std::process::exit(0);
            }
        }
    }
    let _ = std::fs::write(&lock_path, std::process::id().to_string());

    let ctrl_token = generate_ctrl_token();
    // Whether the SDDM toggle is even usable at all (the GUI package doesn't
    // hard-depend on libpam-linux-hello, which is where install-pam.sh lives)
    // — computed once here rather than on every /sddm-status request, since
    // it can't change over this process's lifetime (installing the package
    // mid-run isn't a case worth handling).
    let install_pam_available = std::path::Path::new("/usr/bin/install-pam.sh").exists();
    let ctrl_port = start_control_server(uid, ctrl_token.clone(), install_pam_available);
    eprintln!("🔌 Control server on port {}", ctrl_port);
    // Written to files readable from QML (Qt.environmentVariable unavailable
    // on this build), under $XDG_RUNTIME_DIR (see `runtime_dir`'s doc comment
    // for why not /tmp). 0600: the control server binds 127.0.0.1, reachable
    // by any local process regardless of user — the port number alone isn't
    // sensitive, but the token is what actually gates access to routes like
    // /sddm-enable, which now triggers a real pkexec prompt. A failure here
    // is loud, not swallowed: it means the GUI has no way to reach its own
    // backend, so it's worth knowing about immediately rather than silently
    // limping along with a QML frontend that can never authenticate.
    if let Err(e) = write_owner_only_file(&ctrl_port_path(uid), &ctrl_port.to_string()) {
        eprintln!("❌ Could not write the control port file: {}", e);
    }
    if let Err(e) = write_owner_only_file(&ctrl_token_path(uid), &ctrl_token) {
        eprintln!("❌ Could not write the control token file: {}", e);
    }

    // Configure the QML import paths
    let qml_import_paths = [
        "/usr/lib/x86_64-linux-gnu/qt6/qml",  // Qt6 modules
        "/usr/share/qt6/qml",                 // Standard Qt6 modules
        "/usr/share/linux-hello/qml-modules", // Custom modules
    ]
    .join(":");

    let qt_plugin_paths = [
        "/usr/lib/x86_64-linux-gnu/qt6/plugins",
        "/usr/lib/qt6/plugins",
    ]
    .join(":");

    // Launch qml6. The trailing `-- <uid>` is how the QML side learns its
    // own UID (to find its own namespaced port/token files below):
    // Qt.environmentVariable is unavailable on this build, so LINUX_HELLO_UID
    // isn't readable from QML, but qml6 forwards anything after `--` into
    // Qt.application.arguments, which is. Must stay the last argument, since
    // the QML side reads it by position (the last element).
    let mut cmd = Command::new("qml6");
    cmd.arg("-name")
        .arg("linux-hello")
        .arg(&qml_path)
        .arg("--")
        .arg(uid.to_string())
        .env("LINUX_HELLO_CTRL_PORT", ctrl_port.to_string())
        .env("LINUX_HELLO_UID", uid.to_string())
        .env("QML_IMPORT_PATH", &qml_import_paths)
        .env("QML2_IMPORT_PATH", &qml_import_paths)
        .env("QT_PLUGIN_PATH", &qt_plugin_paths)
        .env("QT_QPA_PLATFORMTHEME", "kde")
        .env("QT_QUICK_CONTROLS_STYLE", "org.kde.desktop")
        .env("QT_APPLICATION_DISPLAY_NAME", "Linux Hello")
        .env("QT_QPA_DESKTOPFILENAME", "linux-hello")
        .env("QML_XHR_ALLOW_FILE_READ", "1")
        .env("QT_QPA_PLATFORM", "xcb;wayland;offscreen")
        .env("QT_STYLE_OVERRIDE", "org.kde.desktop")
        .env("QT_XCB_GL_INTEGRATION", "xcb_egl,none")
        .env("QT_DEBUG_PLUGINS", "0");

    eprintln!("🚀 Launching Linux Hello Configuration GUI");
    eprintln!("  📂 QML path: {}", qml_path);
    eprintln!("  🔧 QML import paths configured");

    match cmd.spawn() {
        Ok(mut child) => {
            // QT_QPA_DESKTOPFILENAME (set above) has no effect on this
            // window: the stock `qml6` runtime stamps its own identity
            // (_KDE_NET_WM_DESKTOP_FILE="org.qt-project.qml") on every
            // window it creates, which is what routes KWin to the generic
            // QML icon instead of ours. Best-effort fixup after the window
            // maps — X11/XWayland only (matches QT_QPA_PLATFORM's "xcb"
            // preference above), silently skipped if `xprop` isn't
            // installed or under a pure-Wayland session.
            thread::spawn(fix_window_desktop_file);
            let _ = child.wait();
        }
        Err(e) => {
            eprintln!("❌ Error while launching: {}", e);
            std::process::exit(1);
        }
    }
}

/// Overrides the `_KDE_NET_WM_DESKTOP_FILE` X property on our just-launched
/// window so KWin resolves the icon via `linux-hello.desktop` (and its
/// `Icon=linux-hello` entry) instead of the generic `qml6` tool's own.
/// Polls briefly since the window isn't mapped the instant the process
/// spawns.
fn fix_window_desktop_file() {
    for _ in 0..20 {
        thread::sleep(std::time::Duration::from_millis(150));

        let Ok(list) = Command::new("xprop")
            .args(["-root", "_NET_CLIENT_LIST"])
            .output()
        else {
            return; // xprop not installed — nothing we can do, not fatal
        };
        let list = String::from_utf8_lossy(&list.stdout);

        for window_id in list.split_whitespace().filter(|s| s.starts_with("0x")) {
            let window_id = window_id.trim_end_matches(',');
            let Ok(class) = Command::new("xprop")
                .args(["-id", window_id, "WM_CLASS"])
                .output()
            else {
                return;
            };
            let class = String::from_utf8_lossy(&class.stdout);
            if !class.contains("\"linux-hello\"") {
                continue;
            }

            let _ = Command::new("xprop")
                .args([
                    "-id",
                    window_id,
                    "-f",
                    "_KDE_NET_WM_DESKTOP_FILE",
                    "8u",
                    "-set",
                    "_KDE_NET_WM_DESKTOP_FILE",
                    "linux-hello",
                ])
                .status();
            return;
        }
    }
}

fn find_qml_path() -> String {
    let candidates = [
        "/usr/share/qt6/qml/Linux/Hello/qml/main.qml",
        "/usr/share/qt6/qml/Linux/Hello/main.qml",
        "/usr/share/linux-hello/qml/main.qml",
        "/usr/share/linux-hello/qml-modules/Linux/Hello/main.qml",
    ];

    for candidate in &candidates {
        if PathBuf::from(candidate).exists() {
            return candidate.to_string();
        }
    }

    // Fallback to the development directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(&manifest_dir)
        .join("qml")
        .join("main.qml")
        .to_string_lossy()
        .to_string()
}

/// Returns the current user's UID.
fn get_current_uid() -> u32 {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1000)
}

/// Base directory for the control server's port/token files: `$XDG_RUNTIME_DIR`
/// (falling back to its standard value, `/run/user/<uid>` — safe to assume
/// for a desktop GUI app launched from a real login session, unlike the
/// sandboxed system service in `hello_daemon::screenlock`, which only ever
/// uses the env var and warns if it's unset). Deliberately not `/tmp`: unlike
/// `/run/user/<uid>` (mode 0700, owned solely by this UID), `/tmp` is shared
/// with every other local user, and its sticky bit only stops *other* users
/// from deleting files *we* own — it does nothing to stop the reverse. A
/// different-UID attacker can pre-create
/// `/tmp/linux-hello-ctrl-<our-uid>.token` themselves; we can then never
/// remove or replace their file (sticky bit: only the owner may unlink it),
/// so our real token is silently never published and the GUI's entire
/// "authenticated" control channel is quietly redirected to whatever
/// port/token the attacker planted — this exact codebase already hit this
/// class of problem once, see `hello_daemon::screenlock::control_port_file_path`.
fn runtime_dir(uid: u32) -> String {
    std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{}", uid))
}

/// Per-UID path for the control server's port file — see `runtime_dir`'s doc
/// comment for why this lives under `$XDG_RUNTIME_DIR` rather than `/tmp`.
fn ctrl_port_path(uid: u32) -> String {
    format!("{}/linux-hello-ctrl.port", runtime_dir(uid))
}

/// Per-UID path for the control server's auth token file (see `ctrl_port_path`).
fn ctrl_token_path(uid: u32) -> String {
    format!("{}/linux-hello-ctrl.token", runtime_dir(uid))
}

/// Generates a random 64-hex-char token for authenticating requests to the
/// local control server. Reads exactly 32 bytes from /dev/urandom directly
/// (`read_exact`, not `fs::read` — the latter would block forever on a
/// character device that never returns EOF) rather than pulling in a `rand`
/// crate: this binary is deliberately dependency-free (see the doc comment
/// at the top of this file).
fn generate_ctrl_token() -> String {
    use std::fs::File;
    let mut buf = [0u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .expect("Unable to read /dev/urandom for the control server token");
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Writes `contents` to `path` readable/writable only by the current user
/// (mode 0600) — used for the control server's port and token files, both
/// of which are otherwise reachable/discoverable by any local process
/// regardless of user (the loopback socket has no per-user ACL of its own).
///
/// Guards against a stale or maliciously pre-planted file already sitting at
/// `path` (e.g. left by a crashed prior run, or a symlink into somewhere
/// else): removes whatever is there first, then creates fresh with
/// `create_new` (`O_CREAT|O_EXCL`), which fails rather than following a
/// symlink or reusing a file we don't control if something wins the (now
/// much narrower) race to recreate it in between. The mode is passed to
/// `open()` itself rather than applied via a separate `set_permissions` call
/// afterward — the latter would leave a window, file existing but not yet
/// locked down, during which another local process could read the
/// freshly-written token.
///
/// This alone is NOT sufficient in a directory shared with other UIDs (like
/// `/tmp`): if a *different* user already owns the file at `path`, the
/// `remove_file` above fails (sticky-bit directories like `/tmp` only let a
/// file's owner unlink it, regardless of the directory's own permissions),
/// `create_new` then fails too since the path still exists, and this
/// function correctly returns `Err` — but only if the caller actually checks
/// that `Err`. Callers of this function for anything security-sensitive
/// must (a) use a directory that isn't shared with other UIDs to begin with
/// (see `runtime_dir`) and (b) not silently ignore a returned `Err`.
fn write_owner_only_file(path: &str, contents: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(contents.as_bytes())
}

/// Extracts the JSON content from a busctl output returning a string type.
/// busctl format: s "[{\"face_id\":\"...\"}]"
fn extract_busctl_json(output: &str) -> Option<String> {
    let trimmed = output.trim();
    let content = trimmed.strip_prefix("s \"")?;
    let content = content.strip_suffix('"').unwrap_or(content);
    Some(content.replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// Extracts a parameter from the query string of the first HTTP line.
/// E.g.: "GET /delete-face?id=abc123 HTTP/1.1" → Some("abc123")
fn extract_query_param(req: &str, param: &str) -> Option<String> {
    let line = req.lines().next()?;
    let search = format!("{}=", param);
    let pos = line.find(&search)?;
    let start = pos + search.len();
    let rest = &line[start..];
    let end = rest.find(['&', ' ', '\r']).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Extracts the face_id from the busctl output of a RegisterFace call.
/// busctl format: s "{\"face_id\":\"face_1000_xxx\", ...}"
fn extract_face_id_from_busctl(output: &str) -> Option<String> {
    let key = "face_id\":\"";
    let start = output.find(key)? + key.len();
    let rest = &output[start..];
    let end = rest.find('"').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Whether a PAM service file already has the linux-hello auth line —
/// the same substring check `install-pam.sh --status` and `lh_configure_service`
/// (in pam-lib.sh) use to decide "already configured".
fn sddm_pam_line_present(contents: &str) -> bool {
    contents.contains("pam_linux_hello")
}

/// Starts a multi-threaded HTTP server on 127.0.0.1 (port allocated by the OS).
/// Each connection is handled in a dedicated thread.
/// Returns the assigned port.
fn start_control_server(uid: u32, token: String, install_pam_available: bool) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Unable to start the control server");
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let token = token.clone();
            thread::spawn(move || {
                handle_ctrl_connection(stream, uid, &token, install_pam_available)
            });
        }
    });

    port
}

/// Extracts the `X-Linux-Hello-Token:` header's value from a raw HTTP
/// request, if present. Manual line scan, matching this file's existing
/// hand-rolled parsing style (no HTTP framework) — same approach as
/// `extract_query_param`.
fn extract_token_header(req: &str) -> Option<&str> {
    req.lines().find_map(|line| {
        line.to_ascii_lowercase()
            .starts_with("x-linux-hello-token:")
            .then(|| line["x-linux-hello-token:".len()..].trim())
    })
}

/// Extracts the HTTP method from the request line (e.g. "GET" from
/// "GET /list-faces HTTP/1.1"). Used to decide the OPTIONS exemption on the
/// actual method rather than a substring match against the whole request —
/// `req.contains("OPTIONS")` would also match that literal text appearing
/// anywhere in a path, query string, or header, letting it smuggle any
/// request past the token check below.
fn request_method(req: &str) -> &str {
    req.lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or("")
}

/// Compares two strings in time that doesn't depend on *where* they first
/// differ — unlike `==`/`!=`, which short-circuits at the first mismatching
/// byte. Used to compare the request's token against the expected one: a
/// naive comparison could in principle let a co-resident local process
/// (without the token, but able to reach the loopback port) recover it
/// faster than brute force by timing repeated guesses. The length check
/// still returns early, but the token's length isn't secret — it's always
/// exactly 64 hex chars by construction (`generate_ctrl_token`).
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

/// Handles an incoming HTTP connection in its own thread.
fn handle_ctrl_connection(
    mut stream: TcpStream,
    uid: u32,
    expected_token: &str,
    install_pam_available: bool,
) {
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);

    // Every route below reaches D-Bus, PAM config, or (for /sddm-enable)
    // pkexec — gate all of them on the shared-secret token, except OPTIONS
    // (a harmless empty-body 200, likely dead CORS-preflight handling real
    // browsers would trigger but QML's XHR doesn't; nothing sensitive on
    // that path). Gated on the actual request method (`request_method`),
    // not a substring match against the raw buffer — a prior version used
    // `req.contains("OPTIONS")` here, which any request could satisfy by
    // having that literal text anywhere in its path/query/headers, bypassing
    // the token check entirely.
    let is_options = request_method(&req) == "OPTIONS";
    let token_ok = extract_token_header(&req)
        .map(|t| constant_time_eq(t, expected_token))
        .unwrap_or(false);
    let (status, body): (&str, String) = if !is_options && !token_ok {
        ("403 Forbidden", String::new())
    } else if is_options {
        ("200 OK", String::new())
    } else if req.contains("/start-capture") {
        // Non-blocking: launches the preview capture in the background
        let _ = Command::new("busctl")
            .args([
                "--user",
                "call",
                "com.linuxhello.FaceAuth",
                "/com/linuxhello/FaceAuth",
                "com.linuxhello.FaceAuth",
                "StartCaptureStream",
                "uut",
                &uid.to_string(),
                "600",
                "25000",
            ])
            .spawn();
        ("200 OK", "OK".to_string())
    } else if req.contains("/register-face") {
        // Blocking: waits for the enrollment to finish and returns the face_id
        let request_json = format!(
            r#"{{"user_id":{},"context":"gui","timeout_ms":10000,"num_samples":5}}"#,
            uid
        );
        match Command::new("busctl")
            .args([
                "--user",
                "call",
                "com.linuxhello.FaceAuth",
                "/com/linuxhello/FaceAuth",
                "com.linuxhello.FaceAuth",
                "RegisterFace",
                "s",
                &request_json,
            ])
            .output()
        {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let face_id =
                    extract_face_id_from_busctl(&stdout).unwrap_or_else(|| "unknown".to_string());
                (
                    "200 OK",
                    format!(r#"{{"ok":true,"face_id":"{}"}}"#, face_id),
                )
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr)
                    .replace('"', "\\\"")
                    .replace('\n', " ");
                eprintln!("✗ RegisterFace busctl stderr: {}", err);
                (
                    "500 Internal Server Error",
                    format!(r#"{{"ok":false,"error":"{}"}}"#, err),
                )
            }
            Err(e) => {
                eprintln!("✗ RegisterFace spawn error: {}", e);
                (
                    "500 Internal Server Error",
                    format!(r#"{{"ok":false,"error":"{}"}}"#, e),
                )
            }
        }
    } else if req.contains("/stop-capture") {
        ("200 OK", "STOPPED".to_string())
    } else if req.contains("/daemon-status") {
        // Cheap D-Bus liveness check for the Home screen's status card: does
        // the per-user session bus currently have an owner for
        // com.linuxhello.FaceAuth? No RegisterFace/Verify call involved, so
        // this can't trigger a camera capture or block on one.
        let active = Command::new("busctl")
            .args([
                "--user",
                "call",
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "NameHasOwner",
                "s",
                "com.linuxhello.FaceAuth",
            ])
            .output()
            .map(|out| {
                out.status.success() && String::from_utf8_lossy(&out.stdout).contains("true")
            })
            .unwrap_or(false);
        ("200 OK", format!(r#"{{"active":{}}}"#, active))
    } else if req.contains("/sddm-status") {
        // No elevation needed: /etc/pam.d/sddm is world-readable, and this
        // is exactly what `install-pam.sh --status` itself checks. Also
        // reports whether the SDDM toggle is even usable at all — the GUI
        // package doesn't hard-depend on libpam-linux-hello (install-pam.sh
        // lives there), so a GUI-only install must degrade gracefully
        // instead of offering a button that can never work. `available`
        // is computed once at startup (see `main`), not re-derived here
        // on every request — unlike `active`, it can't change over this
        // process's lifetime.
        let active = std::fs::read_to_string("/etc/pam.d/sddm")
            .map(|contents| sddm_pam_line_present(&contents))
            .unwrap_or(false);
        (
            "200 OK",
            format!(
                r#"{{"active":{},"available":{}}}"#,
                active, install_pam_available
            ),
        )
    } else if req.contains("/sddm-enable") || req.contains("/sddm-disable") {
        // Blocking is fine: each connection already runs on its own thread.
        // pkexec shows the native polkit auth-agent dialog (matched to our
        // action via install-pam.sh's exec-path annotation in
        // com.linuxhello.pam-setup.policy) and waits for it — can take
        // several seconds while the user actually looks at the prompt.
        let flag = if req.contains("/sddm-enable") {
            "--enable-sddm"
        } else {
            "--disable-sddm"
        };
        match Command::new("pkexec")
            .args(["/usr/bin/install-pam.sh", flag])
            .output()
        {
            Ok(out) if out.status.success() => ("200 OK", r#"{"ok":true}"#.to_string()),
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr)
                    .replace('"', "\\\"")
                    .replace('\n', " ");
                eprintln!("✗ {} failed: {}", flag, err);
                ("200 OK", format!(r#"{{"ok":false,"error":"{}"}}"#, err))
            }
            Err(e) => {
                eprintln!("✗ {} spawn error: {}", flag, e);
                ("200 OK", format!(r#"{{"ok":false,"error":"{}"}}"#, e))
            }
        }
    } else if req.contains("/camera-info") {
        // Whether the active camera has an IR channel — see
        // hello_daemon::dbus::camera_info's doc comment. No liveness gate
        // without one, so the Enrollment screen warns the user their setup
        // is more spoofable than the common (IR-equipped) case.
        match Command::new("busctl")
            .args([
                "--user",
                "call",
                "com.linuxhello.FaceAuth",
                "/com/linuxhello/FaceAuth",
                "com.linuxhello.FaceAuth",
                "CameraInfo",
            ])
            .output()
        {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let json = extract_busctl_json(&stdout)
                    .unwrap_or_else(|| r#"{"has_ir":true}"#.to_string());
                ("200 OK", json)
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr)
                    .replace('"', "\\\"")
                    .replace('\n', " ");
                eprintln!("✗ CameraInfo busctl stderr: {}", err);
                // Fail open toward "has IR" rather than flashing a false
                // security warning if the daemon is briefly unreachable —
                // the actual liveness gate isn't affected either way, this
                // route only feeds an informational banner.
                ("200 OK", r#"{"has_ir":true}"#.to_string())
            }
            Err(e) => {
                eprintln!("✗ CameraInfo spawn error: {}", e);
                ("200 OK", r#"{"has_ir":true}"#.to_string())
            }
        }
    } else if req.contains("/list-faces") {
        match Command::new("busctl")
            .args([
                "--user",
                "call",
                "com.linuxhello.FaceAuth",
                "/com/linuxhello/FaceAuth",
                "com.linuxhello.FaceAuth",
                "ListFaces",
                "u",
                &uid.to_string(),
            ])
            .output()
        {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let json = extract_busctl_json(&stdout).unwrap_or_else(|| "[]".to_string());
                ("200 OK", json)
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr)
                    .replace('"', "\\\"")
                    .replace('\n', " ");
                eprintln!("✗ ListFaces busctl stderr: {}", err);
                ("200 OK", "[]".to_string())
            }
            Err(e) => {
                eprintln!("✗ ListFaces spawn error: {}", e);
                ("200 OK", "[]".to_string())
            }
        }
    } else if req.contains("/delete-face") {
        let face_id = extract_query_param(&req, "id").unwrap_or_default();
        let request_json = format!(r#"{{"user_id":{},"face_id":"{}"}}"#, uid, face_id);
        match Command::new("busctl")
            .args([
                "--user",
                "call",
                "com.linuxhello.FaceAuth",
                "/com/linuxhello/FaceAuth",
                "com.linuxhello.FaceAuth",
                "DeleteFace",
                "s",
                &request_json,
            ])
            .output()
        {
            Ok(out) if out.status.success() => ("200 OK", r#"{"ok":true}"#.to_string()),
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr)
                    .replace('"', "\\\"")
                    .replace('\n', " ");
                eprintln!("✗ DeleteFace busctl stderr: {}", err);
                (
                    "500 Internal Server Error",
                    format!(r#"{{"ok":false,"error":"{}"}}"#, err),
                )
            }
            Err(e) => {
                eprintln!("✗ DeleteFace spawn error: {}", e);
                (
                    "500 Internal Server Error",
                    format!(r#"{{"ok":false,"error":"{}"}}"#, e),
                )
            }
        }
    } else {
        ("404 Not Found", String::new())
    };

    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
        status,
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sddm_pam_line_present_detects_configured_line() {
        let contents = "#%PAM-1.0\n\
            auth    requisite       pam_nologin.so\n\
            # >>> linux-hello-start\n\
            auth       sufficient   pam_linux_hello.so context=sddm\n\
            # <<< linux-hello-end\n\
            @include common-auth\n";
        assert!(sddm_pam_line_present(contents));
    }

    #[test]
    fn sddm_pam_line_present_false_for_stock_file() {
        let contents = "#%PAM-1.0\n\
            auth    requisite       pam_nologin.so\n\
            @include common-auth\n";
        assert!(!sddm_pam_line_present(contents));
    }

    #[test]
    fn sddm_pam_line_present_false_for_empty_string() {
        assert!(!sddm_pam_line_present(""));
    }

    #[test]
    fn extract_token_header_finds_value_case_insensitively() {
        let req =
            "GET /list-faces HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Linux-Hello-Token: abc123\r\n\r\n";
        assert_eq!(extract_token_header(req), Some("abc123"));

        let req_lower = "GET /list-faces HTTP/1.1\r\nx-linux-hello-token: abc123\r\n\r\n";
        assert_eq!(extract_token_header(req_lower), Some("abc123"));
    }

    #[test]
    fn extract_token_header_preserves_value_case() {
        // The header *name* match is case-insensitive, but the token
        // *value* itself (hex, technically case-insensitive too, but the
        // comparison must still be exact) must come through unmodified.
        let req = "GET / HTTP/1.1\r\nX-Linux-Hello-Token: AbC123\r\n\r\n";
        assert_eq!(extract_token_header(req), Some("AbC123"));
    }

    #[test]
    fn extract_token_header_none_when_missing() {
        let req = "GET /list-faces HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(extract_token_header(req), None);
    }

    #[test]
    fn constant_time_eq_matches_regular_equality() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(constant_time_eq("", ""));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc123", "abc12"));
        assert!(!constant_time_eq("abc12", "abc123"));
        assert!(!constant_time_eq("abc123", ""));
    }

    #[test]
    fn constant_time_eq_is_case_sensitive() {
        // The token is hex but stored/compared verbatim — this must not
        // silently accept a case-differing guess.
        assert!(!constant_time_eq("AbC123", "abc123"));
    }

    #[test]
    fn request_method_extracts_the_real_method() {
        assert_eq!(request_method("GET /list-faces HTTP/1.1\r\n\r\n"), "GET");
        assert_eq!(request_method("OPTIONS / HTTP/1.1\r\n\r\n"), "OPTIONS");
        assert_eq!(request_method(""), "");
    }

    #[test]
    fn request_method_is_not_fooled_by_options_appearing_elsewhere() {
        // A prior version gated the token check on `req.contains("OPTIONS")`
        // against the whole raw request, which this exact input would have
        // bypassed: the literal text "OPTIONS" appears in the query string
        // of a GET request, but the method itself is GET, not OPTIONS.
        let req = "GET /sddm-enable?x=OPTIONS HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(request_method(req), "GET");
        assert_ne!(request_method(req), "OPTIONS");
    }

    #[test]
    fn generate_ctrl_token_is_64_lowercase_hex_chars_and_varies() {
        let a = generate_ctrl_token();
        let b = generate_ctrl_token();
        assert_eq!(a.len(), 64);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_ne!(a, b, "two calls must not produce the same token");
    }

    #[test]
    fn ctrl_paths_live_under_the_runtime_dir_and_are_distinct() {
        // Doesn't assert the exact literal path — that depends on whether
        // XDG_RUNTIME_DIR happens to be set in the environment this test
        // runs in, and mutating it here would race with other tests running
        // in parallel in the same process. What matters is: both files live
        // under this UID's runtime dir (not /tmp — see `runtime_dir`'s doc
        // comment for why), and the two filenames don't collide.
        let dir = runtime_dir(1000);
        assert_eq!(
            ctrl_port_path(1000),
            format!("{}/linux-hello-ctrl.port", dir)
        );
        assert_eq!(
            ctrl_token_path(1000),
            format!("{}/linux-hello-ctrl.token", dir)
        );
        assert_ne!(ctrl_port_path(1000), ctrl_token_path(1000));
        assert!(!ctrl_port_path(1000).starts_with("/tmp/"));
    }

    #[test]
    fn write_owner_only_file_sets_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!(
            "linux-hello-test-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path_str = path.to_str().unwrap();
        write_owner_only_file(path_str, "secret").unwrap();
        let mode = std::fs::metadata(path_str).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(path_str).unwrap(), "secret");
        let _ = std::fs::remove_file(path_str);
    }

    #[test]
    fn write_owner_only_file_replaces_a_preexisting_permissive_file() {
        // Simulates a stale file left behind by a crashed prior run (or a
        // same-UID process that got here first) — the new content and 0600
        // mode must win regardless of what was there before. This does NOT
        // cover a different-UID attacker pre-planting the path in a shared,
        // sticky directory like /tmp (create_new's remove-then-recreate
        // can't remove a file it doesn't own); that's why ctrl_port_path/
        // ctrl_token_path use $XDG_RUNTIME_DIR instead, not this function.
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!(
            "linux-hello-test-preexisting-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path_str = path.to_str().unwrap();
        std::fs::write(path_str, "planted content").unwrap();
        std::fs::set_permissions(path_str, std::fs::Permissions::from_mode(0o666)).unwrap();

        write_owner_only_file(path_str, "secret").unwrap();

        let mode = std::fs::metadata(path_str).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(path_str).unwrap(), "secret");
        let _ = std::fs::remove_file(path_str);
    }
}
