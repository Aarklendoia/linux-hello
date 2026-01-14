// Build script pour linux_hello_config
// Utilise Qt/QML pour une UI native KDE Wayland

fn main() {
    // On doit avoir Qt et Kirigami d'installés
    // Les commandes à exécuter sont:
    // sudo apt install qml-module-org-kde-kirigami qt6-qml-private

    println!("cargo:rerun-if-changed=qml/");
    println!("cargo:rerun-if-changed=build.rs");

    // Indique à Qt/QML où trouver les modules
    // Ceci est complété à runtime dans main.rs
}
