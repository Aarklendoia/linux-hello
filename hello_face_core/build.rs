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

// det_500m.onnx et w600k_mbf.onnx ne sont plus publiés comme assets individuels
// depuis la release v0.7 : ils sont packagés dans l'archive buffalo_sc.zip.
const MODEL_PACK_URL: &str =
    "https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_sc.zip";

const MODELS: &[(&str, &str)] = &[
    ("det_500m.onnx", "detection SCRFD-500M"),
    ("w600k_mbf.onnx", "embedding ArcFace MobileNetV3"),
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

fn download_pack(url: &str, dest: &PathBuf) -> bool {
    println!("cargo:warning=Téléchargement du pack de modèles: {}", url);

    // Essayer curl d'abord, puis wget
    for (cmd, args) in &[
        (
            "curl",
            vec![
                "-L",
                "--silent",
                "--max-time",
                "60",
                "--output",
                dest.to_str().unwrap(),
                url,
            ],
        ),
        ("wget", vec!["-q", "-O", dest.to_str().unwrap(), url]),
    ] {
        if let Ok(status) = Command::new(cmd).args(args).status() {
            if status.success() {
                // Vérifier que le fichier a une taille minimale (>1MB = pas une page d'erreur)
                if dest.metadata().map(|m| m.len() > 1_000_000).unwrap_or(false) {
                    println!("cargo:warning=✓ Pack de modèles téléchargé");
                    return true;
                }
            }
        }
        let _ = std::fs::remove_file(dest);
    }

    println!("cargo:warning=⚠ Impossible de télécharger le pack de modèles - le fallback stub sera utilisé");
    false
}

fn extract_from_pack(zip_path: &PathBuf, filename: &str, dest: &PathBuf, desc: &str) -> bool {
    let dir = dest.parent().unwrap();
    let status = Command::new("unzip")
        .args([
            "-o",
            "-j",
            zip_path.to_str().unwrap(),
            filename,
            "-d",
            dir.to_str().unwrap(),
        ])
        .status();

    if matches!(status, Ok(s) if s.success())
        && dest.metadata().map(|m| m.len() > 10_000).unwrap_or(false)
    {
        println!("cargo:warning=✓ Modèle {} extrait", desc);
        true
    } else {
        println!(
            "cargo:warning=⚠ Impossible d'extraire {} - le fallback stub sera utilisé",
            desc
        );
        let _ = std::fs::remove_file(dest);
        false
    }
}

fn main() {
    // En CI, sauter le téléchargement des modèles (le stub sera utilisé).
    if std::env::var("LINUX_HELLO_NO_MODEL_DOWNLOAD").is_ok() {
        println!("cargo:warning=⚠ LINUX_HELLO_NO_MODEL_DOWNLOAD défini, téléchargement ignoré");
        println!("cargo:rustc-env=LINUX_HELLO_MODELS_DIR=/tmp/linux-hello-models-ci");
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }

    let dir = models_dir();

    if let Err(e) = std::fs::create_dir_all(&dir) {
        println!(
            "cargo:warning=⚠ Impossible de créer le dossier modèles {}: {}",
            dir.display(),
            e
        );
        return;
    }

    // Un fichier de taille nulle est un reliquat d'un téléchargement échoué : à retenter.
    let present = |filename: &str| dir.join(filename).metadata().map(|m| m.len() > 0).unwrap_or(false);

    let missing: Vec<_> = MODELS.iter().filter(|(f, _)| !present(f)).collect();

    if missing.is_empty() {
        for (_, desc) in MODELS {
            println!("cargo:warning=✓ Modèle {} déjà présent", desc);
        }
    } else {
        let zip_path = std::env::temp_dir().join("linux-hello-buffalo_sc.zip");
        if download_pack(MODEL_PACK_URL, &zip_path) {
            for (filename, desc) in &missing {
                let dest = dir.join(filename);
                extract_from_pack(&zip_path, filename, &dest, desc);
            }
        }
        let _ = std::fs::remove_file(&zip_path);
    }

    // Exposer le chemin des modèles comme variable d'environnement de compile
    println!("cargo:rustc-env=LINUX_HELLO_MODELS_DIR={}", dir.display());

    // Re-exécuter si les modèles changent
    println!("cargo:rerun-if-changed=build.rs");
}
