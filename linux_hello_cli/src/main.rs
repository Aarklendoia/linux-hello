//! CLI de test pour Linux Hello
//!
//! Commandes de développement sans PAM:
//! - linux-hello daemon      : lancer le daemon
//! - linux-hello enroll $UID : enregistrer un visage
//! - linux-hello verify $UID : tester une vérification
//! - linux-hello list $UID   : lister les visages enregistrés

use clap::{Parser, Subcommand};
use tracing::{info, Level};

#[derive(Parser)]
#[command(name = "linux-hello")]
#[command(about = "CLI - Linux Hello Face Authentication", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Niveau de verbosité
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Lancer le daemon
    Daemon {
        /// Mode debug
        #[arg(short, long)]
        debug: bool,

        /// Chemin de stockage custom
        #[arg(short, long)]
        storage: Option<std::path::PathBuf>,
    },

    /// Enregistrer un nouveau visage
    Enroll {
        /// UID de l'utilisateur
        user_id: u32,

        /// Contexte (login, sudo, screenlock, etc.)
        #[arg(short, long, default_value = "test")]
        context: String,

        /// Nombre de samples à prendre
        #[arg(short, long, default_value = "3")]
        samples: u32,
    },

    /// Tester la vérification
    Verify {
        /// UID de l'utilisateur
        user_id: u32,

        /// Contexte
        #[arg(short, long, default_value = "test")]
        context: String,

        /// Timeout en ms
        #[arg(short, long, default_value = "5000")]
        timeout: u64,
    },

    /// Lister les visages enregistrés
    List {
        /// UID de l'utilisateur
        user_id: u32,
    },

    /// Supprimer tous les visages d'un utilisateur
    Delete {
        /// UID
        user_id: u32,

        /// ID spécifique du visage (optionnel)
        face_id: Option<String>,
    },

    /// Tester la caméra
    Camera {
        /// Durée du test en secondes
        #[arg(short, long, default_value = "5")]
        duration: u64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialiser tracing
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
    info!("Lancement du daemon");

    let mut config = hello_daemon::DaemonConfig::default();
    if let Some(path) = storage {
        config.storage_path = path;
    }
    config.debug = debug;

    let _daemon = hello_daemon::FaceAuthDaemon::new(config)?;

    info!("Daemon prêt - Ctrl+C pour arrêter");
    tokio::signal::ctrl_c().await?;
    info!("Arrêt du daemon");

    Ok(())
}

async fn command_enroll(user_id: u32, context: &str, samples: u32) -> anyhow::Result<()> {
    info!(
        "Enregistrement d'un visage pour UID {} (contexte: {})",
        user_id, context
    );
    info!("Nombre de samples: {}", samples);

    // TODO: Appeler le daemon D-Bus
    info!("Non implémenté - À connecter au daemon D-Bus");

    Ok(())
}

async fn command_verify(user_id: u32, context: &str, timeout: u64) -> anyhow::Result<()> {
    info!(
        "Vérification de l'utilisateur {} (contexte: {})",
        user_id, context
    );
    info!("Timeout: {}ms", timeout);

    // TODO: Appeler le daemon D-Bus
    info!("Non implémenté - À connecter au daemon D-Bus");

    Ok(())
}

async fn command_list(user_id: u32) -> anyhow::Result<()> {
    info!("Listage des visages enregistrés pour UID {}", user_id);

    // TODO: Appeler le daemon D-Bus
    info!("Non implémenté - À connecter au daemon D-Bus");

    Ok(())
}

async fn command_delete(user_id: u32, face_id: Option<String>) -> anyhow::Result<()> {
    if let Some(id) = face_id {
        info!("Suppression du visage {} pour UID {}", id, user_id);
    } else {
        info!("Suppression de TOUS les visages pour UID {}", user_id);
    }

    // TODO: Appeler le daemon D-Bus
    info!("Non implémenté - À connecter au daemon D-Bus");

    Ok(())
}

async fn command_camera(duration: u64) -> anyhow::Result<()> {
    info!("Test caméra pendant {}s", duration);

    let config = hello_camera::CameraConfig::default();
    let mut camera = hello_camera::create_camera(config)?;

    camera.open()?;
    info!("Caméra ouverte: {}", camera.backend_name());

    let start = std::time::Instant::now();
    let mut frame_count = 0;

    while start.elapsed().as_secs() < duration {
        match camera.capture(1000) {
            Ok(frame) => {
                frame_count += 1;
                info!(
                    "Frame {}: {}x{}, taille={}B",
                    frame_count,
                    frame.width,
                    frame.height,
                    frame.data.len()
                );
            }
            Err(e) => {
                eprintln!("Erreur capture: {}", e);
                break;
            }
        }
    }

    camera.close()?;
    info!("Test terminé: {} frames capturées", frame_count);

    Ok(())
}
