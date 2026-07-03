//! Test CLI for Linux Hello
//!
//! Development commands without PAM:
//! - linux-hello daemon      : launch the daemon
//! - linux-hello enroll $UID : enroll a face
//! - linux-hello verify $UID : test a verification
//! - linux-hello list $UID   : list enrolled faces

use clap::{Parser, Subcommand};
use tracing::{info, Level};

#[derive(Parser)]
#[command(name = "linux-hello")]
#[command(about = "CLI - Linux Hello Face Authentication", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbosity level
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the daemon
    Daemon {
        /// Debug mode
        #[arg(short, long)]
        debug: bool,

        /// Custom storage path
        #[arg(short, long)]
        storage: Option<std::path::PathBuf>,
    },

    /// Enroll a new face
    Enroll {
        /// User UID
        user_id: u32,

        /// Context (login, sudo, screenlock, etc.)
        #[arg(short, long, default_value = "test")]
        context: String,

        /// Number of samples to take
        #[arg(short, long, default_value = "3")]
        samples: u32,
    },

    /// Test the verification
    Verify {
        /// User UID
        user_id: u32,

        /// Context
        #[arg(short, long, default_value = "test")]
        context: String,

        /// Timeout in ms
        #[arg(short, long, default_value = "5000")]
        timeout: u64,
    },

    /// List enrolled faces
    List {
        /// User UID
        user_id: u32,
    },

    /// Delete all faces for a user
    Delete {
        /// UID
        user_id: u32,

        /// Specific face ID (optional)
        face_id: Option<String>,
    },

    /// Test the camera
    Camera {
        /// Test duration in seconds
        #[arg(short, long, default_value = "5")]
        duration: u64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let level = if cli.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    tracing_subscriber::fmt().with_max_level(level).init();

    info!("Linux Hello CLI v{}", env!("CARGO_PKG_VERSION"));

    match cli.command {
        Commands::Daemon { debug, storage } => command_daemon(debug, storage).await,
        Commands::Enroll {
            user_id,
            context,
            samples,
        } => command_enroll(user_id, &context, samples).await,
        Commands::Verify {
            user_id,
            context,
            timeout,
        } => command_verify(user_id, &context, timeout).await,
        Commands::List { user_id } => command_list(user_id).await,
        Commands::Delete { user_id, face_id } => command_delete(user_id, face_id).await,
        Commands::Camera { duration } => command_camera(duration).await,
    }
}

async fn command_daemon(debug: bool, storage: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    info!("Starting daemon");

    let mut config = hello_daemon::DaemonConfig::default();
    if let Some(path) = storage {
        config.storage_path = path;
    }
    config.debug = debug;

    let _daemon = hello_daemon::FaceAuthDaemon::new(config)?;

    info!("Daemon ready - Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;
    info!("Stopping daemon");

    Ok(())
}

async fn command_enroll(user_id: u32, context: &str, samples: u32) -> anyhow::Result<()> {
    info!(
        "Enrolling a face for UID {} (context: {})",
        user_id, context
    );
    info!("Number of samples: {}", samples);

    // TODO: Call the D-Bus daemon
    info!("Not implemented - To be connected to the D-Bus daemon");

    Ok(())
}

async fn command_verify(user_id: u32, context: &str, timeout: u64) -> anyhow::Result<()> {
    info!("Verifying user {} (context: {})", user_id, context);
    info!("Timeout: {}ms", timeout);

    // TODO: Call the D-Bus daemon
    info!("Not implemented - To be connected to the D-Bus daemon");

    Ok(())
}

async fn command_list(user_id: u32) -> anyhow::Result<()> {
    info!("Listing enrolled faces for UID {}", user_id);

    // TODO: Call the D-Bus daemon
    info!("Not implemented - To be connected to the D-Bus daemon");

    Ok(())
}

async fn command_delete(user_id: u32, face_id: Option<String>) -> anyhow::Result<()> {
    if let Some(id) = face_id {
        info!("Deleting face {} for UID {}", id, user_id);
    } else {
        info!("Deleting ALL faces for UID {}", user_id);
    }

    // TODO: Call the D-Bus daemon
    info!("Not implemented - To be connected to the D-Bus daemon");

    Ok(())
}

async fn command_camera(duration: u64) -> anyhow::Result<()> {
    info!("Camera test for {}s", duration);

    let config = hello_camera::CameraConfig::default();
    let mut camera = hello_camera::create_camera(config)?;

    camera.open()?;
    info!("Camera opened: {}", camera.backend_name());

    let start = std::time::Instant::now();
    let mut frame_count = 0;

    while start.elapsed().as_secs() < duration {
        match camera.capture(1000) {
            Ok(frame) => {
                frame_count += 1;
                info!(
                    "Frame {}: {}x{}, size={}B",
                    frame_count,
                    frame.width,
                    frame.height,
                    frame.data.len()
                );
            }
            Err(e) => {
                eprintln!("Capture error: {}", e);
                break;
            }
        }
    }

    camera.close()?;
    info!("Test finished: {} frames captured", frame_count);

    Ok(())
}
