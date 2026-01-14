//! Test CLI du daemon - appels directs aux APIs
//!
//! Simule les interactions avec le daemon sans D-Bus

use clap::{Parser, Subcommand};
use hello_daemon::{DaemonConfig, FaceAuthDaemon};
use std::path::PathBuf;
use tracing::info;

#[derive(Parser)]
#[command(name = "hello-test")]
#[command(about = "Test client pour daemon linux-hello")]
struct Cli {
    /// Chemin du stockage (doit correspondre au daemon)
    #[arg(short, long, default_value = "./hello-test")]
    storage: PathBuf,

    #[command(subcommand)]
    command: Commands,

    /// Mode debug
    #[arg(short, long)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Enregistrer un nouveau visage
    Register {
        /// UID utilisateur
        user_id: u32,
        /// Contexte (login, sudo, screenlock, etc.)
        #[arg(default_value = "test")]
        context: String,
    },

    /// Vérifier l'identité d'un utilisateur
    Verify {
        /// UID utilisateur
        user_id: u32,
        /// Contexte
        #[arg(default_value = "test")]
        context: String,
    },

    /// Lister les visages enregistrés
    List {
        /// UID utilisateur
        user_id: u32,
    },

    /// Supprimer un visage
    Delete {
        /// UID utilisateur
        user_id: u32,
        /// Face ID à supprimer (optionnel, sinon tous)
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

    info!("Test client Linux Hello");
    info!("Stockage: {}", cli.storage.display());

    // Créer config et daemon
    let config = DaemonConfig {
        storage_path: cli.storage.clone(),
        root_mode: false,
        current_uid: None,
        default_similarity_threshold: 0.6,
        debug: cli.debug,
    };

    let daemon = FaceAuthDaemon::new(config)?;

    // Exécuter commande
    match cli.command {
        Commands::Register { user_id, context } => {
            info!(
                "Enregistrement pour user_id={}, context={}",
                user_id, context
            );

            let request = hello_daemon::dbus_interface::RegisterFaceRequest {
                user_id,
                context: context.clone(),
                timeout_ms: 5000,
                num_samples: 3,
            };

            match daemon.register_face(request).await {
                Ok(response_json) => {
                    println!("\n✓ Enregistrement réussi!");
                    println!("Réponse: {}", response_json);
                }
                Err(e) => {
                    eprintln!("\n✗ Erreur enregistrement: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::Verify { user_id, context } => {
            info!("Vérification pour user_id={}, context={}", user_id, context);

            let request = hello_daemon::dbus_interface::VerifyRequest {
                user_id,
                context: context.clone(),
                timeout_ms: 5000,
            };

            match daemon.verify(request).await {
                Ok(result) => {
                    println!("\n✓ Vérification complète");
                    println!("Résultat: {}", result);
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
                            println!("  Meilleur score: {:.4}", best_score);
                            println!("  Seuil requis: {:.4}", threshold);
                        }
                        hello_daemon::dbus_interface::VerifyResult::NoEnrollment => {
                            println!("  Aucun visage enregistré pour cet utilisateur");
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    eprintln!("\n✗ Erreur vérification: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::List { user_id } => {
            info!("Listing visages pour user_id={}", user_id);

            match daemon.list_faces(user_id).await {
                Ok(faces_json) => {
                    println!("\n✓ Visages enregistrés:");
                    println!("{}", faces_json);
                }
                Err(e) => {
                    eprintln!("\n✗ Erreur listing: {}", e);
                    return Err(e.into());
                }
            }
        }

        Commands::Delete { user_id, face_id } => {
            info!("Suppression user_id={}, face_id={:?}", user_id, face_id);

            let request = hello_daemon::dbus_interface::DeleteFaceRequest { user_id, face_id };

            match daemon.delete_face(request).await {
                Ok(_) => {
                    println!("\n✓ Suppression réussie!");
                }
                Err(e) => {
                    eprintln!("\n✗ Erreur suppression: {}", e);
                    return Err(e.into());
                }
            }
        }
    }

    Ok(())
}
