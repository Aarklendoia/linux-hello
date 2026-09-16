//! Client for `pam_helper`'s embedding-relay socket — used only by the
//! per-user `hello-daemon` to give root's `hello-daemon-system` a plaintext
//! copy of one of its own embeddings, so root can seal an independent copy
//! under its own TPM key (see `embedding_cipher`'s module doc for why
//! there are two separate copies at all).
//!
//! Every function here is **best-effort**: `hello-daemon-system`, and this
//! socket, only exist once SDDM face-login has been opted into
//! (`sudo install-pam.sh --enable-sddm`) — most installs never run it. A
//! connection failure (the common case: nothing listening) is logged at
//! `debug!`, never surfaced as an error to the caller — enrollment/deletion
//! on a machine that never enabled SDDM face-login must behave exactly as
//! it did before this feature existed.

use crate::pam_helper::{
    embedding_relay_socket_path, EmbeddingRelayRequest, EmbeddingRelayResponse,
};
use crate::FaceRecord;
use hello_face_core::Embedding;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tracing::{debug, warn};

/// Bounds every connection attempt this module makes — root might be
/// present but briefly unresponsive; this must never make an enroll/delete
/// call (or the startup sync in `main.rs`) hang waiting on it.
const RELAY_TIMEOUT: Duration = Duration::from_secs(2);

async fn send(req: &EmbeddingRelayRequest) -> Result<(), String> {
    let socket_path = embedding_relay_socket_path();
    tokio::time::timeout(RELAY_TIMEOUT, send_inner(&socket_path, req))
        .await
        .map_err(|_| "timed out".to_string())?
}

async fn send_inner(socket_path: &str, req: &EmbeddingRelayRequest) -> Result<(), String> {
    let mut stream = UnixStream::connect(socket_path)
        .await
        .map_err(|e| e.to_string())?;
    let payload = serde_json::to_vec(req).map_err(|e| e.to_string())?;
    stream
        .write_all(&payload)
        .await
        .map_err(|e| e.to_string())?;
    stream.shutdown().await.map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .await
        .map_err(|e| e.to_string())?;
    if buf.is_empty() {
        // hello-daemon-system closing without a response (e.g. it rejected
        // the request before writing one) — treat like any other failure.
        return Err("no response".to_string());
    }
    let resp: EmbeddingRelayResponse = serde_json::from_slice(&buf).map_err(|e| e.to_string())?;
    match resp {
        EmbeddingRelayResponse::Ok => Ok(()),
        EmbeddingRelayResponse::Error { reason } => Err(reason),
    }
}

/// Best-effort push of one embedding to root's own store — call after a
/// successful local `save_face`. Silent on failure (logged at `debug!`
/// only): this is a background sync, not something enroll/verify should
/// ever be gated on.
pub async fn push(record: FaceRecord, embedding: Embedding) {
    let face_id = record.face_id.clone();
    let req = EmbeddingRelayRequest::Push { record, embedding };
    if let Err(e) = send(&req).await {
        debug!(
            "embedding_relay: could not push face_id={} to root (hello-daemon-system likely not running): {}",
            face_id, e
        );
    }
}

/// Best-effort deletion of one face (`Some(face_id)`) or every face
/// (`None`) from root's store — call after a successful local delete.
pub async fn delete(user_id: u32, face_id: Option<String>) {
    let req = EmbeddingRelayRequest::Delete { user_id, face_id };
    if let Err(e) = send(&req).await {
        debug!(
            "embedding_relay: could not relay a deletion for user_id={} to root: {}",
            user_id, e
        );
    }
}

/// Pushes every currently-enrolled face for `user_id` — called once at
/// per-user `hello-daemon` startup (see `main.rs`) so root ends up with a
/// synced copy of faces enrolled before SDDM face-login was ever turned on
/// (root wasn't running to receive the per-enroll push at the time), or
/// faces belonging to a user other than whoever most recently ran
/// `install-pam.sh --enable-sddm`. A no-op, not a warning, if root isn't
/// reachable at all — that's the common case on a machine that never
/// enabled SDDM face-login.
///
/// Reads (and, for any still-plaintext face, opportunistically migrates)
/// through `storage`'s own `load_face_embedding`, so this doubles as a
/// trigger for local encryption migration too, not just the remote sync.
pub async fn sync_all(storage: &crate::storage::FaceStorage, user_id: u32) {
    let faces = match storage.list_user_faces(user_id) {
        Ok(faces) => faces,
        Err(e) => {
            debug!("embedding_relay: could not list faces to sync: {}", e);
            return;
        }
    };
    if faces.is_empty() {
        return;
    }

    // Bound the whole sweep, not just each individual push — a
    // reachable-but-very-slow root must not delay daemon startup
    // indefinitely just because there happen to be many enrolled faces.
    let sweep = async {
        for record in faces {
            let face_id = record.face_id.clone();
            match storage.load_face_embedding(user_id, &face_id) {
                Ok(embedding) => push(record, embedding).await,
                Err(e) => warn!(
                    "embedding_relay: could not read face_id={} to sync: {}",
                    face_id, e
                ),
            }
        }
    };
    if tokio::time::timeout(Duration::from_secs(10), sweep)
        .await
        .is_err()
    {
        warn!("embedding_relay: startup sync to root timed out, will retry on next startup");
    }
}
