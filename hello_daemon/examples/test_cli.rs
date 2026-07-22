//! Daemon test CLI - direct API calls
//!
//! Simulates interactions with the daemon without D-Bus

use clap::{Parser, Subcommand};
use hello_daemon::{DaemonConfig, FaceAuthDaemon};
use std::path::PathBuf;
use tracing::info;

#[derive(Parser)]
#[command(name = "hello-test")]
#[command(about = "Test client for the linux-hello daemon")]
struct Cli {
    /// Storage path (must match the daemon)
    #[arg(short, long, default_value = "./hello-test")]
    storage: PathBuf,

    #[command(subcommand)]
    command: Commands,

    /// Debug mode
    #[arg(short, long)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Register a new face
    Register {
        /// User UID
        user_id: u32,
        /// Context (login, sudo, screenlock, etc.)
        #[arg(default_value = "test")]
        context: String,
    },

    /// Verify a user's identity
    Verify {
        /// User UID
        user_id: u32,
        /// Context
        #[arg(default_value = "test")]
        context: String,
    },

    /// List registered faces
    List {
        /// User UID
        user_id: u32,
    },

    /// Delete a face
    Delete {
        /// User UID
        user_id: u32,
        /// Face ID to delete (optional, otherwise all)
        face_id: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Init logging
    let level = if cli.debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .init();

    info!("Linux Hello test client");
    info!("Storage: {}", cli.storage.display());

    // Create config and daemon
    let config = DaemonConfig {
        storage_path: cli.storage.clone(),
        root_mode: false,
    };

    let daemon = FaceAuthDaemon::new(config)?;

    // Execute command
    match cli.command {
        Commands::Register { user_id, context } => {
            info!("Registering for user_id={}, context={}", user_id, context);

            let request = hello_daemon::dbus_interface::RegisterFaceRequest {
                user_id,
                context: context.clone(),
                timeout_ms: 5000,
                num_samples: 3,
            };

            match daemon.register_face(request).await {
                Ok(response_json) => {
                    println!("\n✓ Registration successful!");
                    println!("Response: {}", response_json);
                }
                Err(e) => {
                    eprintln!("\n✗ Registration error: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::Verify { user_id, context } => {
            info!("Verifying for user_id={}, context={}", user_id, context);

            let request = hello_daemon::dbus_interface::VerifyRequest {
                user_id,
                context: context.clone(),
                timeout_ms: 5000,
            };

            match daemon.verify(request).await {
                Ok(result) => {
                    println!("\n✓ Verification complete");
                    println!("Result: {}", result);
                    match result {
                        hello_daemon::dbus_interface::VerifyResult::Success {
                            face_id,
                            similarity_score,
                        } => {
                            println!("  Face ID: {}", face_id);
                            println!("  Score: {:.4}", similarity_score);
                        }
                        hello_daemon::dbus_interface::VerifyResult::NoMatch {
                            best_score,
                            threshold,
                        } => {
                            println!("  Best score: {:.4}", best_score);
                            println!("  Required threshold: {:.4}", threshold);
                        }
                        hello_daemon::dbus_interface::VerifyResult::NoEnrollment => {
                            println!("  No face registered for this user");
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    eprintln!("\n✗ Verification error: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::List { user_id } => {
            info!("Listing faces for user_id={}", user_id);

            match daemon.list_faces(user_id).await {
                Ok(faces_json) => {
                    println!("\n✓ Registered faces:");
                    println!("{}", faces_json);
                }
                Err(e) => {
                    eprintln!("\n✗ Listing error: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::Delete { user_id, face_id } => {
            info!("Deleting user_id={}, face_id={:?}", user_id, face_id);

            let request = hello_daemon::dbus_interface::DeleteFaceRequest { user_id, face_id };

            match daemon.delete_face(request).await {
                Ok(_) => {
                    println!("\n✓ Deletion successful!");
                }
                Err(e) => {
                    eprintln!("\n✗ Deletion error: {}", e);
                    return Err(e.into());
                }
            }
        }
    }

    Ok(())
}
