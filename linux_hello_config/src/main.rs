//! Linux Hello - Configuration GUI pour KDE/Wayland
//!
//! Lanceur simple Qt6/QML qui:
//! - Lance le moteur QML via qml6
//! - Affiche l'interface de configuration
//! - Intègre l'aperçu vidéo live du daemon
//!
//! Le daemon (hello_daemon) exporte les frames via D-Bus

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Déterminer le chemin QML
    let qml_path = find_qml_path();

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
    cmd.arg(&qml_path)
        .env("QML_IMPORT_PATH", &qml_import_paths)
        .env("QML2_IMPORT_PATH", &qml_import_paths)
        .env("QT_PLUGIN_PATH", &qt_plugin_paths)
        .env("QT_QPA_PLATFORMTHEME", "kde")
        .env("QT_QUICK_CONTROLS_STYLE", "org.kde.desktop")
        .env("QT_APPLICATION_DISPLAY_NAME", "Linux Hello")
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
