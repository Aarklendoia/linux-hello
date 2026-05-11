//! build.rs pour hello_face_core
//!
//! Télécharge les modèles ONNX si absents :
//!   - det_500m.onnx  (SCRFD-500M, ~500 KB)
//!   - w600k_mbf.onnx (ArcFace MobileNetV3, ~2.5 MB)
//!
//! Les modèles sont stockés dans ~/.local/share/linux-hello/models/
//! (ou /var/lib/linux-hello/models/ en mode root).
//!
//! Si le téléchargement échoue, le build continue normalement —
//! le code Rust tombera automatiquement sur le fallback stub.

use std::path::PathBuf;
use std::process::Command;

const MODELS: &[(&str, &str, &str)] = &[
    (
        "det_500m.onnx",
        "https://github.com/deepinsight/insightface/releases/download/v0.7/det_500m.onnx",
        "detection SCRFD-500M",
    ),
    (
        "w600k_mbf.onnx",
        "https://github.com/deepinsight/insightface/releases/download/v0.7/w600k_mbf.onnx",
        "embedding ArcFace MobileNetV3",
    ),
];

fn models_dir() -> PathBuf {
    // Utiliser XDG_DATA_HOME ou fallback HOME
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".local/share"))
                .unwrap_or_else(|_| PathBuf::from("/tmp"))
        });
    base.join("linux-hello/models")
}

fn download(url: &str, dest: &PathBuf, desc: &str) -> bool {
    println!("cargo:warning=Téléchargement modèle {}: {}", desc, url);

    // Essayer curl d'abord, puis wget
    for (cmd, args) in &[
        ("curl", vec!["-L", "--silent", "--output", dest.to_str().unwrap(), url]),
        ("wget", vec!["-q", "-O", dest.to_str().unwrap(), url]),
    ] {
        if let Ok(status) = Command::new(cmd).args(args).status() {
            if status.success() {
                // Vérifier que le fichier a une taille minimale (>10KB = pas une page d'erreur)
                if dest.metadata().map(|m| m.len() > 10_000).unwrap_or(false) {
                    println!("cargo:warning=✓ Modèle {} téléchargé", desc);
                    return true;
                } else {
                    println!("cargo:warning=✗ Fichier trop petit pour {}, suppression", desc);
                    let _ = std::fs::remove_file(dest);
                }
            }
        }
    }

    println!(
        "cargo:warning=⚠ Impossible de télécharger {} - le fallback stub sera utilisé",
        desc
    );
    false
}

fn main() {
    let dir = models_dir();

    if let Err(e) = std::fs::create_dir_all(&dir) {
        println!(
            "cargo:warning=⚠ Impossible de créer le dossier modèles {}: {}",
            dir.display(),
            e
        );
        return;
    }

    for (filename, url, desc) in MODELS {
        let dest = dir.join(filename);
        if dest.exists() {
            println!("cargo:warning=✓ Modèle {} déjà présent", desc);
        } else {
            download(url, &dest, desc);
        }
    }

    // Exposer le chemin des modèles comme variable d'environnement de compile
    println!("cargo:rustc-env=LINUX_HELLO_MODELS_DIR={}", dir.display());

    // Re-exécuter si les modèles changent
    println!("cargo:rerun-if-changed=build.rs");
}
