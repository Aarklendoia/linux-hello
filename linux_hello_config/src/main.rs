//! Linux Hello - Configuration GUI pour KDE/Wayland
//!
//! Interface graphique QML native avec thème Breeze pour:
//! - Enregistrement de visage avec preview en direct
//! - Configuration des paramètres d'authentification
//! - Gestion des visages enregistrés
//!
//! La logique métier communique via D-Bus avec hello_daemon

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Lance l'application QML avec Kirigami
    // Les fichiers QML sont dans le répertoire 'qml/'

    // Déterminer le chemin QML (système ou développement)
    let qml_path =
        if PathBuf::from("/usr/share/linux-hello/qml-modules/Linux/Hello/main.qml").exists() {
            "/usr/share/linux-hello/qml-modules/Linux/Hello/main.qml".to_string()
        } else {
            let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(&manifest_dir)
                .join("qml")
                .join("main.qml")
                .to_string_lossy()
                .to_string()
        };

    // Configurer les chemins d'import QML (Qt6 uniquement, pas Qt5)
    // IMPORTANT: qml6 nécessite que les chemins soient dans le bon ordre
    let qml_import_paths = [
        "/usr/lib/x86_64-linux-gnu/qt6/qml",  // Qt6 modules principaux
        "/usr/share/linux-hello/qml-modules", // ✨ Modules personnalisés
    ]
    .join(":");

    let qt_plugin_paths = [
        "/usr/lib/x86_64-linux-gnu/qt6/plugins",
        "/usr/lib/qt6/plugins",
    ]
    .join(":");

    // Configuration pour VM/graphics virtuel
    let mut cmd = Command::new("qml6");
    cmd.arg(&qml_path)
        // Chemins des modules QML (CRITIQUE pour Kirigami)
        .env("QML_IMPORT_PATH", &qml_import_paths)
        .env("QML2_IMPORT_PATH", &qml_import_paths)
        // Chemins des plugins Qt
        .env("QT_PLUGIN_PATH", &qt_plugin_paths)
        // Platform theme (Qt5)
        .env("QT_QPA_PLATFORMTHEME", "kde")
        // Style des contrôles (Kirigami)
        .env("QT_QUICK_CONTROLS_STYLE", "org.kde.desktop")
        // Métadonnées d'application
        .env("QT_APPLICATION_DISPLAY_NAME", "Linux Hello")
        // Permettre XHR sur fichiers locaux (i18n)
        .env("QML_XHR_ALLOW_FILE_READ", "1")
        // Wayland avec fallback X11/offscreen
        .env("QT_QPA_PLATFORM", "xcb;wayland;offscreen")
        // Force le style Breeze KDE
        .env("QT_STYLE_OVERRIDE", "org.kde.desktop")
        // XCB avec GPU si possible
        .env("QT_XCB_GL_INTEGRATION", "xcb_egl,none")
        // Désactive les avertissements de driver
        .env("QT_DEBUG_PLUGINS", "0")
        // Supprime les messages de binding loop connus de Kirigami ToolTip
        .env("QML_BIND_IGNORE", "1");

    eprintln!("🚀 Launching Linux Hello Configuration GUI");
    eprintln!("  📂 QML path: {}", qml_path);
    eprintln!("  🔧 QML import paths configured");

    match cmd.spawn() {
        Ok(mut child) => {
            let _ = child.wait();
        }
        Err(e) => {
            eprintln!("❌ Erreur lors du lancement de l'application QML : {}", e);
            eprintln!("   Vérifie que 'qml6' est installé : sudo apt install qml-qt6");
            std::process::exit(1);
        }
    }
}
