//! Daemon d'authentification faciale - point d'entrée
//!
//! Lance le service D-Bus pour gestion des visages

use clap::Parser;
use hello_daemon::{DaemonConfig, FaceAuthDaemon};
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "hello-daemon")]
#[command(about = "Linux Hello - Daemon d'authentification faciale", long_about = None)]
struct Args {
    /// Chemin de stockage des embeddings
    #[arg(short, long)]
    storage_path: Option<PathBuf>,

    /// Mode debug
    #[arg(short, long)]
    debug: bool,

    /// Seuil de similarité (0.0-1.0)
    #[arg(long, default_value = "0.6")]
    similarity_threshold: f32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialiser tracing
    let level = if args.debug { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .init();

    info!("Démarrage du daemon Linux Hello");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Créer config du daemon
    let mut config = DaemonConfig::default();
    if let Some(path) = args.storage_path {
        config.storage_path = path;
    }
    config.debug = args.debug;
    config.default_similarity_threshold = args.similarity_threshold;

    info!("Stockage: {}", config.storage_path.display());
    info!("Seuil similarité: {}", config.default_similarity_threshold);

    // Créer le daemon
    let daemon = FaceAuthDaemon::new(config)?;

    if daemon.config().root_mode {
        info!("Mode root activé - accessible pour tous les utilisateurs");
    } else {
        warn!("Mode user - accessible uniquement pour l'utilisateur courant");
    }

    // TODO: Se connecter à D-Bus et exposer l'interface
    info!("D-Bus interface non implémentée yet");
    info!("À faire: com.linuxhello.FaceAuth");

    // Garder le daemon actif indéfiniment
    tokio::signal::ctrl_c().await?;
    info!("Arrêt du daemon");

    Ok(())
}
