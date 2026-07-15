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

    let ctrl_port = start_control_server(uid);
    eprintln!("🔌 Control server on port {}", ctrl_port);
    // Write the port to a file readable from QML (Qt.environmentVariable unavailable on this build)
    let _ = std::fs::write("/tmp/linux-hello-ctrl.port", ctrl_port.to_string());

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

    // Launch qml6
    let mut cmd = Command::new("qml6");
    cmd.arg("-name")
        .arg("linux-hello")
        .arg(&qml_path)
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

/// Starts a multi-threaded HTTP server on 127.0.0.1 (port allocated by the OS).
/// Each connection is handled in a dedicated thread.
/// Returns the assigned port.
fn start_control_server(uid: u32) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Unable to start the control server");
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || handle_ctrl_connection(stream, uid));
        }
    });

    port
}

/// Handles an incoming HTTP connection in its own thread.
fn handle_ctrl_connection(mut stream: TcpStream, uid: u32) {
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);

    let (status, body): (&str, String) = if req.contains("/start-capture") {
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
            .map(|out| out.status.success() && String::from_utf8_lossy(&out.stdout).contains("true"))
            .unwrap_or(false);
        ("200 OK", format!(r#"{{"active":{}}}"#, active))
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
    } else if req.contains("OPTIONS") {
        ("200 OK", String::new())
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
