//! Facial authentication daemon - entry point
//!
//! Launches the D-Bus service for face management

use clap::Parser;
use hello_daemon::{dbus::FaceAuthInterface, DaemonConfig, FaceAuthDaemon};
use std::path::PathBuf;
use tracing::{error, info, warn};

extern crate libc;

#[derive(Parser, Debug)]
#[command(name = "hello-daemon")]
#[command(about = "Linux Hello - Facial authentication daemon", long_about = None)]
struct Args {
    /// Path for storing embeddings
    #[arg(short, long)]
    storage_path: Option<PathBuf>,

    /// Debug mode
    #[arg(short, long)]
    debug: bool,

    /// Similarity threshold (0.0-1.0)
    #[arg(long, default_value = "0.6")]
    similarity_threshold: f32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize tracing
    let level = if args.debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .init();

    info!("Starting Linux Hello daemon");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Create the daemon config
    let mut config = DaemonConfig::default();
    if let Some(path) = args.storage_path {
        config.storage_path = path;
    }
    config.debug = args.debug;
    config.default_similarity_threshold = args.similarity_threshold;

    info!("Storage: {}", config.storage_path.display());
    info!(
        "Similarity threshold: {}",
        config.default_similarity_threshold
    );

    // Create the daemon
    let daemon = FaceAuthDaemon::new(config)?;
    let storage_path = daemon.config().storage_path.to_string_lossy().into_owned();

    if daemon.config().root_mode {
        info!("Root mode enabled - accessible to all users");
    } else {
        warn!("User mode - accessible only to the current user");
    }

    // Wrap in Arc<RwLock> for sharing with the PAM helper
    let daemon_arc = std::sync::Arc::new(tokio::sync::RwLock::new(daemon));

    // Start the PAM helper socket (/run/hello-pam/<uid>.socket)
    let uid = unsafe { libc::getuid() };
    if let Err(e) = hello_daemon::pam_helper::start_pam_helper(uid, daemon_arc.clone()).await {
        warn!(
            "PAM helper socket not started: {} (biometric PAM auth unavailable)",
            e
        );
    } else {
        info!("✓ PAM helper socket: /run/hello-pam/{}.socket", uid);
    }

    // Start monitoring the screen lock (automatic unlock via face), sharing
    // live status + a retry trigger with the control server below so
    // qml/lockscreen/MainBlock.qml can show progress and let the user
    // retry on demand instead of getting exactly one attempt per lock.
    let screenlock_status = std::sync::Arc::new(std::sync::Mutex::new(
        hello_daemon::screenlock::ScreenlockStatus::default(),
    ));
    let screenlock_retry_notify = std::sync::Arc::new(tokio::sync::Notify::new());

    if let Err(e) = hello_daemon::screenlock::start_screenlock_watcher(
        daemon_arc.clone(),
        uid,
        screenlock_status.clone(),
        screenlock_retry_notify.clone(),
    )
    .await
    {
        warn!(
            "Screen lock monitoring not started: {} (automatic unlock unavailable)",
            e
        );
    } else {
        info!("✓ Screen lock monitoring active");
    }

    if let Err(e) = hello_daemon::screenlock::start_screenlock_control_server(
        screenlock_status,
        screenlock_retry_notify,
    )
    .await
    {
        warn!(
            "Screenlock control server not started: {} (status/retry UI in the lock screen unavailable)",
            e
        );
    } else {
        info!("✓ Screenlock control server active");
    }

    // Start the MJPEG server for the GUI preview (real-time video stream)
    hello_daemon::preview::start_mjpeg_server().await?;

    // Register on D-Bus
    info!("Registering on D-Bus...");

    let connection = zbus::Connection::session().await.map_err(|e| {
        error!("D-Bus connection error: {}", e);
        e
    })?;

    let iface = FaceAuthInterface::from_arc(daemon_arc, storage_path, connection.clone());

    connection
        .request_name("com.linuxhello.FaceAuth")
        .await
        .map_err(|e| {
            error!("D-Bus name registration error: {}", e);
            e
        })?;

    connection
        .object_server()
        .at("/com/linuxhello/FaceAuth", iface)
        .await
        .map_err(|e| {
            error!("D-Bus object registration error: {}", e);
            e
        })?;

    info!("✓ D-Bus service registered: com.linuxhello.FaceAuth");
    info!("  Interface: /com/linuxhello/FaceAuth");
    info!("  Methods: register_face, verify, delete_face, list_faces, ping");
    info!("  Signals: CaptureProgress, CaptureCompleted, CaptureError");

    // Keep the daemon running indefinitely
    info!("Daemon ready. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    info!("Stopping daemon");

    Ok(())
}
