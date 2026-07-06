//! Test CLI for Linux Hello
//!
//! Development commands without PAM:
//! - linux-hello daemon      : launch the daemon
//! - linux-hello enroll $UID : enroll a face
//! - linux-hello verify $UID : test a verification
//! - linux-hello list $UID   : list enrolled faces

use clap::{Parser, Subcommand};
use hello_daemon::dbus_interface::{
    DeleteFaceRequest, RegisterFaceRequest, RegisterFaceResponse, VerifyRequest, VerifyResult,
};
use hello_daemon::FaceRecord;
use tracing::{info, Level};
use zbus::Connection;

const DBUS_DEST: &str = "com.linuxhello.FaceAuth";
const DBUS_PATH: &str = "/com/linuxhello/FaceAuth";
const DBUS_INTERFACE: &str = "com.linuxhello.FaceAuth";

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

/// Connect to the daemon's D-Bus service. `zbus::Proxy::new` binds lazily and
/// succeeds even if the daemon isn't running — the actual failure only
/// surfaces on the first method call, which `daemon_call_error` below turns
/// into a friendlier hint.
async fn daemon_proxy(conn: &Connection) -> anyhow::Result<zbus::Proxy<'_>> {
    Ok(zbus::Proxy::new(conn, DBUS_DEST, DBUS_PATH, DBUS_INTERFACE).await?)
}

/// Wrap a failed D-Bus call with a hint when the cause is "daemon not
/// running" — the most common failure mode for this CLI, since
/// `hello-daemon` is a per-user systemd service that isn't started
/// automatically.
fn daemon_call_error(action: &str, e: zbus::Error) -> anyhow::Error {
    if let zbus::Error::MethodError(name, _, _) = &e {
        if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" {
            return anyhow::anyhow!(
                "{action} failed: the linux-hello daemon isn't running. \
                 Try: systemctl --user status hello-daemon \
                 (or start it: systemctl --user start hello-daemon)"
            );
        }
    }
    anyhow::anyhow!("{action} failed: {e}")
}

async fn command_enroll(user_id: u32, context: &str, samples: u32) -> anyhow::Result<()> {
    info!(
        "Enrolling a face for UID {} (context: {})",
        user_id, context
    );
    info!("Number of samples: {}", samples);

    let conn = Connection::session().await?;
    let proxy = daemon_proxy(&conn).await?;

    let request = RegisterFaceRequest {
        user_id,
        context: context.to_string(),
        timeout_ms: 10_000,
        num_samples: samples,
    };
    let request_json = serde_json::to_string(&request)?;

    let response_json: String = proxy
        .call("RegisterFace", &(request_json,))
        .await
        .map_err(|e| daemon_call_error("Enrollment", e))?;
    let response: RegisterFaceResponse = serde_json::from_str(&response_json)?;

    println!(
        "✓ Face enrolled: face_id={} quality={:.2}",
        response.face_id, response.quality_score
    );

    Ok(())
}

async fn command_verify(user_id: u32, context: &str, timeout: u64) -> anyhow::Result<()> {
    info!("Verifying user {} (context: {})", user_id, context);
    info!("Timeout: {}ms", timeout);

    let conn = Connection::session().await?;
    let proxy = daemon_proxy(&conn).await?;

    let request = VerifyRequest {
        user_id,
        context: context.to_string(),
        timeout_ms: timeout,
    };
    let request_json = serde_json::to_string(&request)?;

    let result_json: String = proxy
        .call("Verify", &(request_json,))
        .await
        .map_err(|e| daemon_call_error("Verification", e))?;
    let result: VerifyResult = serde_json::from_str(&result_json)?;

    match &result {
        VerifyResult::Success { .. } => println!("✓ {}", result),
        _ => println!("✗ {}", result),
    }

    Ok(())
}

async fn command_list(user_id: u32) -> anyhow::Result<()> {
    info!("Listing enrolled faces for UID {}", user_id);

    let conn = Connection::session().await?;
    let proxy = daemon_proxy(&conn).await?;

    let faces_json: String = proxy
        .call("ListFaces", &(user_id,))
        .await
        .map_err(|e| daemon_call_error("ListFaces", e))?;
    let faces: Vec<FaceRecord> = serde_json::from_str(&faces_json)?;

    if faces.is_empty() {
        println!("No faces enrolled for UID {}", user_id);
    } else {
        println!("{} face(s) enrolled for UID {}:", faces.len(), user_id);
        for f in &faces {
            println!(
                "  {}  context={}  quality={:.2}  registered_at={}",
                f.face_id, f.context, f.quality_score, f.registered_at
            );
        }
    }

    Ok(())
}

async fn command_delete(user_id: u32, face_id: Option<String>) -> anyhow::Result<()> {
    if let Some(id) = &face_id {
        info!("Deleting face {} for UID {}", id, user_id);
    } else {
        info!("Deleting ALL faces for UID {}", user_id);
    }

    let conn = Connection::session().await?;
    let proxy = daemon_proxy(&conn).await?;

    let request = DeleteFaceRequest { user_id, face_id };
    let request_json = serde_json::to_string(&request)?;

    proxy
        .call::<_, _, ()>("DeleteFace", &(request_json,))
        .await
        .map_err(|e| daemon_call_error("DeleteFace", e))?;

    println!("✓ Deleted");

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
