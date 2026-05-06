//! Daemon d'authentification faciale - point d'entrée
//!
//! Lance le service D-Bus pour gestion des visages

use clap::Parser;
use hello_daemon::{dbus::FaceAuthInterface, DaemonConfig, FaceAuthDaemon};
use std::path::PathBuf;
use tracing::{error, info, warn};

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

    // Démarrer le serveur MJPEG pour la preview GUI (flux vidéo temps réel)
    hello_daemon::preview::start_mjpeg_server().await?;

    // Enregistrer sur D-Bus
    info!("Enregistrement sur D-Bus...");

    let connection = zbus::Connection::session().await.map_err(|e| {
        error!("Erreur connexion D-Bus: {}", e);
        e
    })?;

    let iface = FaceAuthInterface::new_with_connection(daemon, connection.clone());

    connection
        .request_name("com.linuxhello.FaceAuth")
        .await
        .map_err(|e| {
            error!("Erreur enregistrement D-Bus name: {}", e);
            e
        })?;

    connection
        .object_server()
        .at("/com/linuxhello/FaceAuth", iface)
        .await
        .map_err(|e| {
            error!("Erreur enregistrement objet D-Bus: {}", e);
            e
        })?;

    info!("✓ Service D-Bus enregistré: com.linuxhello.FaceAuth");
    info!("  Interface: /com/linuxhello/FaceAuth");
    info!("  Méthodes: register_face, verify, delete_face, list_faces, ping");
    info!("  Signaux: CaptureProgress, CaptureCompleted, CaptureError");

    // Garder le daemon actif indéfiniment
    info!("Daemon prêt. Appuyez sur Ctrl+C pour arrêter.");
    tokio::signal::ctrl_c().await?;
    info!("Arrêt du daemon");

    Ok(())
}
