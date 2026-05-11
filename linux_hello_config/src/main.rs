//! Linux Hello - Configuration GUI pour KDE/Wayland
//!
//! Lanceur simple Qt6/QML qui:
//! - Lance le moteur QML via qml6
//! - Affiche l'interface de configuration
//! - Intègre l'aperçu vidéo live du daemon
//!
//! Le daemon (hello_daemon) exporte les frames via D-Bus

use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::thread;

fn main() {
    // Déterminer le chemin QML
    let qml_path = find_qml_path();

    let uid = get_current_uid();

    // Interdire plusieurs instances simultanées
    let lock_path = format!("/tmp/linux-hello-config-{}.lock", uid);
    if let Ok(content) = std::fs::read_to_string(&lock_path) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            if std::path::Path::new(&format!("/proc/{}", pid)).exists() {
                eprintln!(
                    "⚠ Linux Hello est déjà ouvert (PID {}). Une seule instance autorisée.",
                    pid
                );
                std::process::exit(0);
            }
        }
    }
    let _ = std::fs::write(&lock_path, std::process::id().to_string());

    let ctrl_port = start_control_server(uid);
    eprintln!("🔌 Serveur de contrôle sur port {}", ctrl_port);
    // Écrire le port dans un fichier lisible depuis QML (Qt.environmentVariable indisponible sur ce build)
    let _ = std::fs::write("/tmp/linux-hello-ctrl.port", ctrl_port.to_string());

    // Configurer les chemins d'import QML
    let qml_import_paths = [
        "/usr/lib/x86_64-linux-gnu/qt6/qml",  // Qt6 modules
        "/usr/share/qt6/qml",                 // Qt6 standards
        "/usr/share/linux-hello/qml-modules", // Modules custom
    ]
    .join(":");

    let qt_plugin_paths = [
        "/usr/lib/x86_64-linux-gnu/qt6/plugins",
        "/usr/lib/qt6/plugins",
    ]
    .join(":");

    // Lancer qml6
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
            let _ = child.wait();
        }
        Err(e) => {
            eprintln!("❌ Erreur lors du lancement: {}", e);
            std::process::exit(1);
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

    // Fallback vers le répertoire de développement
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(&manifest_dir)
        .join("qml")
        .join("main.qml")
        .to_string_lossy()
        .to_string()
}

/// Retourne l'UID de l'utilisateur courant.
fn get_current_uid() -> u32 {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1000)
}

/// Extrait le contenu JSON depuis une sortie busctl renvoyant un type string.
/// Format busctl : s "[{\"face_id\":\"...\"}]"
fn extract_busctl_json(output: &str) -> Option<String> {
    let trimmed = output.trim();
    let content = trimmed.strip_prefix("s \"")?;
    let content = content.strip_suffix('"').unwrap_or(content);
    Some(content.replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// Extrait un paramètre de la query string de la première ligne HTTP.
/// Ex: "GET /delete-face?id=abc123 HTTP/1.1" → Some("abc123")
fn extract_query_param(req: &str, param: &str) -> Option<String> {
    let line = req.lines().next()?;
    let search = format!("{}=", param);
    let pos = line.find(&search)?;
    let start = pos + search.len();
    let rest = &line[start..];
    let end = rest.find(['&', ' ', '\r']).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Extrait le face_id depuis la sortie busctl d'un appel RegisterFace.
/// Format busctl : s "{\"face_id\":\"face_1000_xxx\", ...}"
fn extract_face_id_from_busctl(output: &str) -> Option<String> {
    let key = "face_id\":\"";
    let start = output.find(key)? + key.len();
    let rest = &output[start..];
    let end = rest.find('"').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Démarre un serveur HTTP multi-threadé sur 127.0.0.1 (port alloué par l'OS).
/// Chaque connexion est traitée dans un thread dédié.
/// Retourne le port attribué.
fn start_control_server(uid: u32) -> u16 {
    let listener =
        TcpListener::bind("127.0.0.1:0").expect("Impossible de démarrer le serveur de contrôle");
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || handle_ctrl_connection(stream, uid));
        }
    });

    port
}

/// Traite une connexion HTTP entrante dans son propre thread.
fn handle_ctrl_connection(mut stream: TcpStream, uid: u32) {
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);

    let (status, body): (&str, String) = if req.contains("/start-capture") {
        // Non-bloquant : lance la capture preview en arrière-plan
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
        // Bloquant : attend la fin de l'enregistrement et retourne le face_id
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
    } else if req.contains("/test-auth") {
        // Teste l'authentification sans PAM : appelle Verify via D-Bus et retourne le résultat
        let context = extract_query_param(&req, "context").unwrap_or_else(|| "gui".to_string());
        let request_json = format!(
            r#"{{"user_id":{},"context":"{}","timeout_ms":10000}}"#,
            uid, context
        );
        match Command::new("busctl")
            .args([
                "--user",
                "call",
                "com.linuxhello.FaceAuth",
                "/com/linuxhello/FaceAuth",
                "com.linuxhello.FaceAuth",
                "Verify",
                "s",
                &request_json,
            ])
            .output()
        {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // busctl retourne : s "{"Success":...}" — extraire le JSON interne
                let json = extract_busctl_json(&stdout).unwrap_or_else(|| {
                    r#"{"result":"Error","message":"Réponse vide"}"#.to_string()
                });
                ("200 OK", format!(r#"{{"ok":true,"data":{}}}"#, json))
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr)
                    .replace('"', "\\\"")
                    .replace('\n', " ");
                eprintln!("✗ Verify busctl stderr: {}", err);
                ("200 OK", format!(r#"{{"ok":false,"error":"{}"}}"#, err))
            }
            Err(e) => {
                eprintln!("✗ Verify spawn error: {}", e);
                ("200 OK", format!(r#"{{"ok":false,"error":"{}"}}"#, e))
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
