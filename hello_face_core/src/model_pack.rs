//! Where the ONNX models live, and where they come from.
//!
//! Linux Hello doesn't ship its face models (#211): `det_500m.onnx` and
//! `w600k_mbf.onnx` come from InsightFace's `buffalo_sc` pack, whose terms
//! only allow non-commercial research use and don't grant redistribution.
//! Each machine fetches them itself, after its admin has seen those terms:
//! `sudo linux-hello models install` (or the settings GUI, which runs the
//! same command through pkexec) downloads the pack from InsightFace's own
//! GitHub release, checks it against [`MODEL_PACK_SHA256`] and extracts the
//! two files into [`SYSTEM_MODELS_DIR`], readable by both the per-user
//! daemon and the root `hello-daemon-system`.

use std::path::{Path, PathBuf};

/// InsightFace's v0.7 release asset holding both models. Mirrored by
/// `hello_face_core/build.rs` (development builds only).
pub const MODEL_PACK_URL: &str =
    "https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_sc.zip";

/// sha256 of [`MODEL_PACK_URL`], verified against the real v0.7
/// `buffalo_sc.zip` on 2026-07-22. A download that doesn't match is
/// rejected: these files end up deciding who gets `sudo`.
pub const MODEL_PACK_SHA256: &str =
    "57d31b56b6ffa911c8a73cfc1707c73cab76efe7f13b675a05223bf42de47c72";

/// The two files the daemons load, as named inside the pack.
pub const MODEL_FILES: [&str; 2] = ["det_500m.onnx", "w600k_mbf.onnx"];

/// Where `linux-hello models install` puts them.
pub const SYSTEM_MODELS_DIR: &str = "/var/lib/linux-hello/models";

/// Where the `linux-hello-models` package put them before #211. Still read,
/// so a machine that kept those files keeps working.
pub const LEGACY_PACKAGED_MODELS_DIR: &str = "/usr/share/linux-hello/models";

/// Where InsightFace states its terms.
pub const LICENSE_URL: &str = "https://github.com/deepinsight/insightface#license";

/// The terms shown before downloading, by the CLI and (translated) by the
/// GUI. Quotes InsightFace's README rather than paraphrasing it.
pub const LICENSE_NOTICE: &str = "\
Linux Hello needs two face models from InsightFace (det_500m.onnx and
w600k_mbf.onnx, from the buffalo_sc pack). Linux Hello does not ship them:
InsightFace's README states that

  \"The training data containing the annotation (and the models trained
  with these data) are available for non-commercial research purposes
  only.\"

and that models downloaded from its GitHub repository \"follow the above
license policy(which is for non-commercial research purposes only)\".
For other uses, InsightFace asks to contact
recognition-oss-pack@insightface.ai for licensing.

Full terms: https://github.com/deepinsight/insightface#license

Downloading them means you accept those terms for this machine. If your use
isn't covered (for example, commercial use), don't download them.";

/// Whether `dir` holds both model files.
pub fn models_present(dir: &Path) -> bool {
    MODEL_FILES.iter().all(|f| dir.join(f).is_file())
}

/// The directory the daemons load models from: the first of
/// `$LINUX_HELLO_MODELS_DIR` (test/development override, taken as-is),
/// [`SYSTEM_MODELS_DIR`], [`LEGACY_PACKAGED_MODELS_DIR`], the build-time
/// download directory (development builds) and `$XDG_DATA_HOME/linux-hello/models`
/// that holds both files — or [`SYSTEM_MODELS_DIR`] if none does, since
/// that's where they'll be installed.
pub fn resolve_models_dir() -> PathBuf {
    if let Ok(p) = std::env::var("LINUX_HELLO_MODELS_DIR") {
        return PathBuf::from(p);
    }
    let mut candidates = vec![
        PathBuf::from(SYSTEM_MODELS_DIR),
        PathBuf::from(LEGACY_PACKAGED_MODELS_DIR),
    ];
    if let Some(p) = option_env!("LINUX_HELLO_MODELS_DIR") {
        candidates.push(PathBuf::from(p));
    }
    if let Some(data_dir) = dirs::data_dir() {
        candidates.push(data_dir.join("linux-hello/models"));
    }
    first_with_models(&candidates).unwrap_or_else(|| PathBuf::from(SYSTEM_MODELS_DIR))
}

fn first_with_models(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|d| models_present(d)).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_models(dir: &Path) {
        for f in MODEL_FILES {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
    }

    #[test]
    fn models_present_needs_both_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!models_present(dir.path()));
        std::fs::write(dir.path().join(MODEL_FILES[0]), b"x").unwrap();
        assert!(!models_present(dir.path()));
        std::fs::write(dir.path().join(MODEL_FILES[1]), b"x").unwrap();
        assert!(models_present(dir.path()));
    }

    #[test]
    fn first_with_models_skips_incomplete_directories() {
        let empty = tempfile::tempdir().unwrap();
        let full = tempfile::tempdir().unwrap();
        with_models(full.path());
        let candidates = [empty.path().to_path_buf(), full.path().to_path_buf()];
        assert_eq!(
            first_with_models(&candidates),
            Some(full.path().to_path_buf())
        );
    }

    #[test]
    fn first_with_models_none_when_nothing_installed() {
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(first_with_models(&[empty.path().to_path_buf()]), None);
    }

    #[test]
    fn the_pinned_hash_is_a_sha256() {
        assert_eq!(MODEL_PACK_SHA256.len(), 64);
        assert!(MODEL_PACK_SHA256.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
