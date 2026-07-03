//! build.rs for hello_face_core
//!
//! Downloads the ONNX models if missing:
//!   - det_500m.onnx  (SCRFD-500M, ~500 KB)
//!   - w600k_mbf.onnx (ArcFace MobileNetV3, ~2.5 MB)
//!
//! Models are stored in ~/.local/share/linux-hello/models/
//! (or /var/lib/linux-hello/models/ in root mode).
//!
//! If the download fails, the build continues normally —
//! the Rust code will automatically fall back to the stub.

use std::path::PathBuf;
use std::process::Command;

// det_500m.onnx and w600k_mbf.onnx are no longer published as individual assets
// since release v0.7: they are packaged in the buffalo_sc.zip archive.
const MODEL_PACK_URL: &str =
    "https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_sc.zip";

const MODELS: &[(&str, &str)] = &[
    ("det_500m.onnx", "detection SCRFD-500M"),
    ("w600k_mbf.onnx", "embedding ArcFace MobileNetV3"),
];

fn models_dir() -> PathBuf {
    // Use XDG_DATA_HOME or fall back to HOME
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
    println!("cargo:warning=Downloading model pack: {}", url);

    // Try curl first, then wget
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
                // Check that the file has a minimum size (>1MB = not an error page)
                if dest.metadata().map(|m| m.len() > 1_000_000).unwrap_or(false) {
                    println!("cargo:warning=✓ Model pack downloaded");
                    return true;
                }
            }
        }
        let _ = std::fs::remove_file(dest);
    }

    println!("cargo:warning=⚠ Failed to download the model pack - stub fallback will be used");
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
        println!("cargo:warning=✓ Model {} extracted", desc);
        true
    } else {
        println!(
            "cargo:warning=⚠ Failed to extract {} - stub fallback will be used",
            desc
        );
        let _ = std::fs::remove_file(dest);
        false
    }
}

fn main() {
    // In CI, skip downloading the models (the stub will be used).
    if std::env::var("LINUX_HELLO_NO_MODEL_DOWNLOAD").is_ok() {
        println!("cargo:warning=⚠ LINUX_HELLO_NO_MODEL_DOWNLOAD set, download skipped");
        println!("cargo:rustc-env=LINUX_HELLO_MODELS_DIR=/tmp/linux-hello-models-ci");
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }

    let dir = models_dir();

    if let Err(e) = std::fs::create_dir_all(&dir) {
        println!(
            "cargo:warning=⚠ Failed to create models directory {}: {}",
            dir.display(),
            e
        );
        return;
    }

    // A zero-size file is a leftover from a failed download: retry it.
    let present = |filename: &str| dir.join(filename).metadata().map(|m| m.len() > 0).unwrap_or(false);

    let missing: Vec<_> = MODELS.iter().filter(|(f, _)| !present(f)).collect();

    if missing.is_empty() {
        for (_, desc) in MODELS {
            println!("cargo:warning=✓ Model {} already present", desc);
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

    // Expose the models path as a compile-time environment variable
    println!("cargo:rustc-env=LINUX_HELLO_MODELS_DIR={}", dir.display());

    // Re-run if the models change
    println!("cargo:rerun-if-changed=build.rs");
}
