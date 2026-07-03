//! Linux Hello - Configuration GUI for KDE/Wayland
//!
//! Native QML graphical interface with Breeze theme for:
//! - Face enrollment with live preview
//! - Authentication settings configuration
//! - Management of enrolled faces
//!
//! Business logic communicates via D-Bus with hello_daemon

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Launch the QML application with Kirigami
    // The QML files are in the 'qml/' directory

    // Determine the QML path (system or development)
    let qml_path = if PathBuf::from("/usr/share/qt6/qml/Linux/Hello/qml/main.qml").exists() {
        "/usr/share/qt6/qml/Linux/Hello/qml/main.qml".to_string()
    } else if PathBuf::from("/usr/share/qt6/qml/Linux/Hello/main.qml").exists() {
        "/usr/share/qt6/qml/Linux/Hello/main.qml".to_string()
    } else if PathBuf::from("/usr/share/linux-hello/qml/main.qml").exists() {
        "/usr/share/linux-hello/qml/main.qml".to_string()
    } else if PathBuf::from("/usr/share/linux-hello/qml-modules/Linux/Hello/main.qml").exists() {
        "/usr/share/linux-hello/qml-modules/Linux/Hello/main.qml".to_string()
    } else {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(&manifest_dir)
            .join("qml")
            .join("main.qml")
            .to_string_lossy()
            .to_string()
    };

    // Configure the QML import paths (Qt6 only, not Qt5)
    // IMPORTANT: qml6 requires the paths to be in the correct order
    let qml_import_paths = [
        "/usr/lib/x86_64-linux-gnu/qt6/qml",  // Main Qt6 modules
        "/usr/share/qt6/qml",                 // ✨ Standard Qt6 modules
        "/usr/share/linux-hello/qml-modules", // ✨ Custom modules
    ]
    .join(":");

    let qt_plugin_paths = [
        "/usr/lib/x86_64-linux-gnu/qt6/plugins",
        "/usr/lib/qt6/plugins",
    ]
    .join(":");

    // Configuration for VM/virtual graphics
    let mut cmd = Command::new("qml6");
    cmd.arg(&qml_path)
        // QML module paths (CRITICAL for Kirigami)
        .env("QML_IMPORT_PATH", &qml_import_paths)
        .env("QML2_IMPORT_PATH", &qml_import_paths)
        // Qt plugin paths
        .env("QT_PLUGIN_PATH", &qt_plugin_paths)
        // Platform theme (Qt5)
        .env("QT_QPA_PLATFORMTHEME", "kde")
        // Controls style (Kirigami)
        .env("QT_QUICK_CONTROLS_STYLE", "org.kde.desktop")
        // Application metadata
        .env("QT_APPLICATION_DISPLAY_NAME", "Linux Hello")
        // Allow XHR on local files (i18n)
        .env("QML_XHR_ALLOW_FILE_READ", "1")
        // Wayland with X11/offscreen fallback
        .env("QT_QPA_PLATFORM", "xcb;wayland;offscreen")
        // Force the KDE Breeze style
        .env("QT_STYLE_OVERRIDE", "org.kde.desktop")
        // XCB with GPU if possible
        .env("QT_XCB_GL_INTEGRATION", "xcb_egl,none")
        // Disable driver warnings
        .env("QT_DEBUG_PLUGINS", "0")
        // Suppress known Kirigami ToolTip binding loop messages
        .env("QML_BIND_IGNORE", "1");

    eprintln!("🚀 Launching Linux Hello Configuration GUI");
    eprintln!("  📂 QML path: {}", qml_path);
    eprintln!("  🔧 QML import paths configured");

    match cmd.spawn() {
        Ok(mut child) => {
            let _ = child.wait();
        }
        Err(e) => {
            eprintln!("❌ Error while launching the QML application: {}", e);
            eprintln!("   Check that 'qml6' is installed: sudo apt install qml-qt6");
            std::process::exit(1);
        }
    }
}
