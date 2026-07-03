// Build script for linux_hello_config
// Uses Qt/QML for a native KDE Wayland UI

fn main() {
    // Qt and Kirigami must be installed
    // The commands to run are:
    // sudo apt install qml-module-org-kde-kirigami qt6-qml-private

    println!("cargo:rerun-if-changed=qml/");
    println!("cargo:rerun-if-changed=build.rs");

    // Tells Qt/QML where to find the modules
    // This is completed at runtime in main.rs
}
