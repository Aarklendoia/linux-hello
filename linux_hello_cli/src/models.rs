//! `linux-hello models status|install` — the face models aren't shipped
//! (#211, see `hello_face_core::model_pack`): this shows InsightFace's terms,
//! and on acceptance downloads the pack from InsightFace's own release,
//! checks its pinned sha256 and installs the two files into
//! `/var/lib/linux-hello/models`.
//!
//! Shells out to `curl` (or `wget`), `sha256sum` and `unzip`, like
//! `hello_face_core/build.rs` already does, rather than pulling an HTTP
//! client and a zip crate into the CLI for a one-off download.

use anyhow::{bail, Context};
use hello_face_core::model_pack::{
    models_present, resolve_models_dir, LICENSE_NOTICE, MODEL_FILES, MODEL_PACK_SHA256,
    MODEL_PACK_URL, SYSTEM_MODELS_DIR,
};
use std::io::{BufRead, IsTerminal, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn status() -> anyhow::Result<()> {
    let dir = resolve_models_dir();
    if models_present(&dir) {
        println!("✓ Face models installed: {}", dir.display());
    } else {
        println!("✗ Face models not installed (expected in {SYSTEM_MODELS_DIR})");
        println!(
            "  Face authentication is disabled until they are: sudo linux-hello models install"
        );
    }
    Ok(())
}

pub fn install(accept_license: bool) -> anyhow::Result<()> {
    let target = PathBuf::from(SYSTEM_MODELS_DIR);
    if models_present(&target) {
        println!("✓ Face models already installed: {}", target.display());
        return Ok(());
    }
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        bail!("installing the face models needs root: sudo linux-hello models install");
    }

    println!("{LICENSE_NOTICE}\n");
    if !accept_license && !ask_acceptance()? {
        bail!("terms not accepted, nothing downloaded");
    }

    let staging = target.with_file_name(".models-download");
    let result = download_and_install(&staging, &target);
    let _ = std::fs::remove_dir_all(&staging);
    result?;

    println!("✓ Face models installed in {}", target.display());
    println!("  The daemons pick them up on the next authentication, no restart needed.");
    Ok(())
}

/// `true` only on an explicit "y". Without a terminal to ask on, refuses:
/// scripts must say `--accept-license` themselves.
fn ask_acceptance() -> anyhow::Result<bool> {
    if !std::io::stdin().is_terminal() {
        bail!("no terminal to confirm on: re-run with --accept-license to accept the terms above");
    }
    print!("Accept these terms and download the models? [y/N]: ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(answer.trim().eq_ignore_ascii_case("y"))
}

fn download_and_install(staging: &Path, target: &Path) -> anyhow::Result<()> {
    // Same filesystem as the target, so the final rename is atomic: the
    // daemons never see a half-written model.
    let _ = std::fs::remove_dir_all(staging);
    std::fs::create_dir_all(staging).with_context(|| format!("creating {}", staging.display()))?;
    std::fs::set_permissions(staging, std::fs::Permissions::from_mode(0o700))?;

    let zip = staging.join("buffalo_sc.zip");
    println!("Downloading {MODEL_PACK_URL} ...");
    download(MODEL_PACK_URL, &zip)?;

    let actual = sha256_hex(&zip)?;
    if !checksum_matches(&actual) {
        bail!("checksum mismatch for the downloaded pack (got {actual}, expected {MODEL_PACK_SHA256}), refusing to install it");
    }
    println!("✓ Checksum verified");

    let status = Command::new("unzip")
        .args(["-o", "-j", "-q"])
        .arg(&zip)
        .args(MODEL_FILES)
        .arg("-d")
        .arg(staging)
        .status()
        .context("running unzip (is it installed?)")?;
    if !status.success() || !models_present(staging) {
        bail!(
            "could not extract {} from the pack",
            MODEL_FILES.join(" and ")
        );
    }

    std::fs::create_dir_all(target).with_context(|| format!("creating {}", target.display()))?;
    std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755))?;
    for file in MODEL_FILES {
        let staged = staging.join(file);
        // Read by the per-user daemon too, not only by root.
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o644))?;
        std::fs::rename(&staged, target.join(file))
            .with_context(|| format!("installing {file} into {}", target.display()))?;
    }
    Ok(())
}

fn download(url: &str, dest: &Path) -> anyhow::Result<()> {
    let attempts: [(&str, Vec<&std::ffi::OsStr>); 2] = [
        (
            "curl",
            vec![
                "--fail".as_ref(),
                "--silent".as_ref(),
                "--show-error".as_ref(),
                "--location".as_ref(),
                "--proto".as_ref(),
                "=https".as_ref(),
                "--max-time".as_ref(),
                "300".as_ref(),
                "--output".as_ref(),
                dest.as_os_str(),
                url.as_ref(),
            ],
        ),
        (
            "wget",
            vec![
                "--https-only".as_ref(),
                "-q".as_ref(),
                "-O".as_ref(),
                dest.as_os_str(),
                url.as_ref(),
            ],
        ),
    ];
    for (cmd, args) in attempts {
        match Command::new(cmd).args(args).status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(_) => bail!("{cmd} could not download {url}"),
            // Not installed: try the next tool.
            Err(_) => continue,
        }
    }
    bail!("neither curl nor wget is installed")
}

fn sha256_hex(path: &Path) -> anyhow::Result<String> {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .context("running sha256sum")?;
    if !output.status.success() {
        bail!("sha256sum failed on {}", path.display());
    }
    String::from_utf8(output.stdout)?
        .split_whitespace()
        .next()
        .map(str::to_string)
        .context("unexpected sha256sum output")
}

fn checksum_matches(actual: &str) -> bool {
    actual.eq_ignore_ascii_case(MODEL_PACK_SHA256)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_must_match_exactly() {
        assert!(checksum_matches(MODEL_PACK_SHA256));
        assert!(checksum_matches(&MODEL_PACK_SHA256.to_uppercase()));
        assert!(!checksum_matches(&MODEL_PACK_SHA256[1..]));
        assert!(!checksum_matches(""));
    }

    #[test]
    fn sha256_hex_of_a_known_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("abc");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(
            sha256_hex(&f).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
