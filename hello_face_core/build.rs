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

use std::path::{Path, PathBuf};
use std::process::Command;

// det_500m.onnx and w600k_mbf.onnx are no longer published as individual assets
// since release v0.7: they are packaged in the buffalo_sc.zip archive.
const MODEL_PACK_URL: &str =
    "https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_sc.zip";

// Pinned so a compromised/tampered release asset can't silently end up
// bundled into a signed package — this exact build.rs pattern also runs
// unattended in debian/scripts/prepare-offline-build.sh (which mirrors this
// constant), feeding the source tarball that publish-ppa.yml signs and
// uploads with no human reviewing the fetched bytes in between. Verified
// against the real v0.7 buffalo_sc.zip with `sha256sum` on 2026-07-22 — if
// upstream ever re-cuts this release asset, both this and the shell script's
// copy need updating together.
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
                                "cargo:warning=⚠ Model pack checksum mismatch (got {}, expected {}) - refusing to use it, stub fallback will be used",
                                actual, MODEL_PACK_SHA256
                            );
                        }
                        None => {
                            println!("cargo:warning=⚠ Could not compute the model pack's checksum (sha256sum missing?) - refusing to use it, stub fallback will be used");
                        }
                    }
                }
            }
        }
        let _ = std::fs::remove_file(dest);
    }

    println!("cargo:warning=⚠ Failed to download the model pack - stub fallback will be used");
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

    // Re-run if the models change
    println!("cargo:rerun-if-changed=build.rs");
}
