//! build.rs for hello_face_core
//!
//! Development convenience: downloads the ONNX models if missing, into
//! $XDG_DATA_HOME/linux-hello/models/ (one of the directories
//! `model_pack::resolve_models_dir` searches):
//!   - det_500m.onnx  (SCRFD-500M)
//!   - w600k_mbf.onnx (ArcFace MobileNetV3)
//!
//! They're InsightFace's, for non-commercial research only (#211), so no
//! package ships them: every packaging build sets
//! LINUX_HELLO_NO_MODEL_DOWNLOAD=1, and installed systems fetch them with
//! `linux-hello models install` (see src/model_pack.rs).
//!
//! If the download fails, the build continues normally; the daemon then
//! refuses to capture until the models are installed.

use std::path::{Path, PathBuf};
use std::process::Command;

// det_500m.onnx and w600k_mbf.onnx are no longer published as individual assets
// since release v0.7: they are packaged in the buffalo_sc.zip archive.
const MODEL_PACK_URL: &str =
    "https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_sc.zip";

// Pinned so a tampered release asset is rejected. Mirrors
// src/model_pack.rs's MODEL_PACK_SHA256 (a build script can't use its own
// crate) — update both together if upstream ever re-cuts this release
// asset. Verified against the real v0.7 buffalo_sc.zip with `sha256sum` on
// 2026-07-22.
const MODEL_PACK_SHA256: &str = "57d31b56b6ffa911c8a73cfc1707c73cab76efe7f13b675a05223bf42de47c72";

const MODELS: &[(&str, &str)] = &[
    ("det_500m.onnx", "detection SCRFD-500M"),
    ("w600k_mbf.onnx", "embedding ArcFace MobileNetV3"),
];

fn models_dir() -> PathBuf {
    // $XDG_DATA_HOME, falling back to ~/.local/share (dirs::data_dir()'s own
    // fallback chain), or /tmp if neither can be determined.
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("linux-hello/models")
}

/// Hex sha256 of `path`, via the `sha256sum` binary (matches this file's
/// existing "std dependencies are sufficient, shell out via Command"
/// approach for `curl`/`wget` — no need for a build-dependency crate just
/// for this).
fn sha256_hex(path: &Path) -> Option<String> {
    let output = Command::new("sha256sum").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .split_whitespace()
        .next()
        .map(str::to_string)
}

fn download_pack(url: &str, dest: &Path) -> bool {
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
                let plausible_size = dest
                    .metadata()
                    .map(|m| m.len() > 1_000_000)
                    .unwrap_or(false);
                if plausible_size {
                    match sha256_hex(dest) {
                        Some(actual) if actual == MODEL_PACK_SHA256 => {
                            println!("cargo:warning=✓ Model pack downloaded and checksum verified");
                            return true;
                        }
                        Some(actual) => {
                            println!(
                                "cargo:warning=⚠ Model pack checksum mismatch (got {}, expected {}) - refusing to use it",
                                actual, MODEL_PACK_SHA256
                            );
                        }
                        None => {
                            println!("cargo:warning=⚠ Could not compute the model pack's checksum (sha256sum missing?) - refusing to use it");
                        }
                    }
                }
            }
        }
        let _ = std::fs::remove_file(dest);
    }

    println!("cargo:warning=⚠ Failed to download the model pack - the daemon will refuse to capture until the models are installed");
    false
}

fn extract_from_pack(zip_path: &Path, filename: &str, dest: &Path, desc: &str) -> bool {
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
            "cargo:warning=⚠ Failed to extract {} - the daemon will refuse to capture until the models are installed",
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
        println!("cargo:rerun-if-env-changed=LINUX_HELLO_NO_MODEL_DOWNLOAD");
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
    let present = |filename: &str| {
        dir.join(filename)
            .metadata()
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    };

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

    // Re-run if the models change or go missing. The models live outside
    // target/, so a target/ restored from a CI cache (or a dev deleting
    // ~/.local/share/linux-hello/models) would otherwise leave this script
    // marked fresh while the files it produced are gone — cargo always
    // re-runs a build script whose rerun-if-changed path doesn't exist
    // (issue #188).
    println!("cargo:rerun-if-changed=build.rs");
    for (filename, _) in MODELS {
        println!("cargo:rerun-if-changed={}", dir.join(filename).display());
    }
    println!("cargo:rerun-if-env-changed=XDG_DATA_HOME");
    println!("cargo:rerun-if-env-changed=LINUX_HELLO_NO_MODEL_DOWNLOAD");
}
