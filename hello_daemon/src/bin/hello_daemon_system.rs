//! Linux Hello - system-wide listener for SDDM (login screen) authentication
//!
//! Started at boot as root, before any user logs in. Deliberately minimal:
//! no D-Bus, no MJPEG preview server, no screenlock watcher, no
//! `FaceAuthDaemon` — just the one Verify-only socket listener that
//! `pam_linux_hello` connects to for `context=sddm` (see
//! `hello_daemon::pam_helper::start_system_pam_helper`). Enrollment always
//! happens through a user's own per-user `hello-daemon` session, never here.

use hello_daemon::camera::CameraManager;
use hello_daemon::matcher::FaceMatcher;
use hello_daemon::pam_helper::start_system_pam_helper;
use std::sync::Arc;
use tracing::{error, info, warn};

const DEFAULT_TIMEOUT_MS: u64 = 5000;
const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.6;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Linux Hello system listener (SDDM login screen)");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    if unsafe { libc::getuid() } != 0 {
        warn!(
            "Not running as root — the system listener will only work for root's own \
             home directory. Run via the packaged hello-daemon-system.service."
        );
    }

    let camera = Arc::new(CameraManager::new(DEFAULT_TIMEOUT_MS));
    let matcher = Arc::new(FaceMatcher::new(DEFAULT_SIMILARITY_THRESHOLD));

    if let Err(e) = start_system_pam_helper(camera, matcher).await {
        error!("Failed to start the system PAM listener: {}", e);
        return Err(anyhow::anyhow!(e.to_string()));
    }
    info!("✓ System PAM listener ready for context=sddm");

    tokio::signal::ctrl_c().await?;
    info!("Stopping system listener");

    Ok(())
}
