//! Persistent storage management for face embeddings
//!
//! - SQLite for metadata (user_id, face_id, quality_score, registered_at)
//! - JSON files for embeddings (for flexibility)
//!
//! # Embedding encryption at rest (#152)
//!
//! Embedding *vectors* (never the `.meta.json` bookkeeping — see
//! `FaceRecord`'s own doc comment) are encrypted with a TPM-sealed key when
//! one is available, falling back to today's plaintext `.embedding.json`
//! when it isn't — enrollment must never fail because of this. See
//! [`crate::embedding_cipher`]'s module doc for the full design: two
//! independent copies (this module's own `.embedding.enc`, and a
//! completely separate one root's `hello-daemon-system` seals for itself
//! under [`root_embeddings_dir`], relayed here via
//! [`push_root_embedding`]/[`delete_root_embedding`] — never read back from
//! this process).

use crate::security_util::write_owner_only_file;
use crate::{embedding_cipher, DaemonError, FaceRecord};
use hello_face_core::Embedding;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Restrict a directory to owner-only access (`0700`).
///
/// Biometric embeddings live under here; `create_dir_all` alone leaves the
/// mode to the process umask (0022 on the systemd units, i.e. world-readable
/// dirs), so this is applied unconditionally after every creation point —
/// including on a dir that already existed from before this hardening
/// landed, so an upgrade self-heals existing installs too.
fn harden_dir(path: &Path) -> Result<(), DaemonError> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| DaemonError::StorageError(format!("chmod 700 on {}: {}", path.display(), e)))
}

/// Which independently-sealed copy of embedding data a [`FaceStorage`]
/// instance reads/writes — see the module doc for why there are two. Never
/// exposed outside this module: callers just get a `FaceStorage` from the
/// right constructor and use it identically either way.
enum EmbeddingSource {
    /// The normal case: a per-user `hello-daemon`'s own storage tree
    /// (`base_path/users/<uid>/<face_id>.embedding.{enc,json}`), or a plain
    /// `open_read_only` for testing/tooling. Encrypted with a key this same
    /// process seals for itself (root, if this process happens to run as
    /// uid 0, e.g. a literal root account's own `hello-daemon`; a
    /// `tpm2-abrmd`-brokered one otherwise).
    OwnPrincipal,
    /// Root's `hello-daemon-system`, verifying a `context=sddm` match for
    /// `uid`. Never reads embeddings from the target user's home directory
    /// at all — only from root's own independently-sealed copy under
    /// [`root_embeddings_dir`], pushed there ahead of time by that user's
    /// own `hello-daemon` (see `pam_helper`'s embedding-relay socket).
    RootRelayStore { uid: u32 },
}

/// Face storage manager
pub struct FaceStorage {
    /// Root storage directory
    base_path: PathBuf,

    /// Path to the SQLite DB
    #[allow(dead_code)]
    db_path: PathBuf,

    embedding_source: EmbeddingSource,
}

impl FaceStorage {
    /// Create a new storage manager
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self, DaemonError> {
        let base_path = base_path.as_ref().to_path_buf();

        // Create the directory structure
        std::fs::create_dir_all(&base_path)
            .map_err(|e| DaemonError::StorageError(format!("Directory creation failed: {}", e)))?;
        harden_dir(&base_path)?;

        let db_path = base_path.join("faces.db");

        let storage = Self {
            base_path,
            db_path,
            embedding_source: EmbeddingSource::OwnPrincipal,
        };

        // Initialize the DB if it doesn't exist
        storage.init_db()?;

        Ok(storage)
    }

    /// Open an existing storage directory without creating anything.
    ///
    /// Returns `Ok(None)` if `base_path` doesn't exist yet (e.g. the user has
    /// never enrolled). Unlike `new()`, this never calls `create_dir_all` —
    /// used by tooling/tests that only need to read an existing tree.
    pub fn open_read_only(base_path: impl AsRef<Path>) -> Result<Option<Self>, DaemonError> {
        let base_path = base_path.as_ref().to_path_buf();
        if !base_path.is_dir() {
            return Ok(None);
        }
        let db_path = base_path.join("faces.db");
        Ok(Some(Self {
            base_path,
            db_path,
            embedding_source: EmbeddingSource::OwnPrincipal,
        }))
    }

    /// Opens `target_home_base` (an arbitrary, not-yet-authenticated user's
    /// home directory) for `context=sddm` verification from root's
    /// `hello-daemon-system` — the one caller allowed to read someone else's
    /// storage tree. Returns `Ok(None)`, creating nothing, if it doesn't
    /// exist yet, same contract as [`open_read_only`](Self::open_read_only).
    ///
    /// Unlike every other constructor, embeddings are **never** read from
    /// `target_home_base` itself here — only `.meta.json` bookkeeping is.
    /// The actual embedding vectors come from root's own independently-
    /// sealed copy (see [`EmbeddingSource::RootRelayStore`]), so a
    /// not-yet-synced face is simply absent from what `load_face_embeddings`
    /// returns, degrading to "no match" rather than reading (or failing to
    /// read) anything under the target user's home.
    pub fn open_read_only_for_system_verify(
        target_home_base: impl AsRef<Path>,
        uid: u32,
    ) -> Result<Option<Self>, DaemonError> {
        let base_path = target_home_base.as_ref().to_path_buf();
        if !base_path.is_dir() {
            return Ok(None);
        }
        let db_path = base_path.join("faces.db");
        Ok(Some(Self {
            base_path,
            db_path,
            embedding_source: EmbeddingSource::RootRelayStore { uid },
        }))
    }

    /// Initialize the SQLite structure
    fn init_db(&self) -> Result<(), DaemonError> {
        // Create the embeddings directory
        let embeddings_dir = self.base_path.join("embeddings");
        std::fs::create_dir_all(&embeddings_dir)
            .map_err(|e| DaemonError::StorageError(format!("Embeddings dir creation: {}", e)))?;
        harden_dir(&embeddings_dir)?;

        // For now we use JSON files
        // Migration to SQLite will happen later with sqlx async
        info!("Storage initialized at: {}", self.base_path.display());

        Ok(())
    }

    /// Save a newly registered face
    pub fn save_face(&self, record: &FaceRecord, embedding: &Embedding) -> Result<(), DaemonError> {
        // Check permissions of the user directory
        let user_dir = self.user_dir(record.user_id)?;
        debug!(
            "save_face: user_id={}, face_id={}, target dir={}",
            record.user_id,
            record.face_id,
            user_dir.display()
        );
        std::fs::create_dir_all(&user_dir).map_err(|e| {
            warn!("save_face: failed to create {}: {}", user_dir.display(), e);
            DaemonError::StorageError(format!("User dir creation: {}", e))
        })?;
        harden_dir(&user_dir)?;

        // Save metadata to a JSON file
        let metadata_path = face_path(&user_dir, &record.face_id, ".meta.json")?;
        let metadata_json =
            serde_json::to_string_pretty(&record).map_err(DaemonError::JsonError)?;

        write_owner_only_file(&metadata_path, &metadata_json).map_err(|e| {
            warn!(
                "save_face: failed to write metadata to {}: {}",
                metadata_path.display(),
                e
            );
            DaemonError::StorageError(format!("write {}: {}", metadata_path.display(), e))
        })?;
        debug!("save_face: metadata written to {}", metadata_path.display());

        // Save the embedding — encrypted under this tree's own TPM-sealed
        // key when one is available, plaintext (as before) otherwise.
        // Enrollment must never fail because of this: any failure getting
        // or using a key just falls back to plaintext rather than
        // propagating an error.
        let is_root = unsafe { libc::getuid() } == 0;
        let key_result = match self.own_embedding_key_path() {
            Ok(path) => embedding_cipher::load_or_create_key(&path, is_root)
                .map_err(|e| DaemonError::StorageError(e.to_string())),
            Err(e) => Err(e),
        };
        match key_result {
            Ok(key) => match embedding_cipher::encrypt_embedding(
                &key,
                embedding,
                record.user_id,
                &record.face_id,
            ) {
                Ok(ciphertext) => {
                    let enc_path = face_path(&user_dir, &record.face_id, ".embedding.enc")?;
                    write_owner_only_file(&enc_path, &ciphertext).map_err(|e| {
                        warn!(
                            "save_face: failed to write encrypted embedding to {}: {}",
                            enc_path.display(),
                            e
                        );
                        DaemonError::StorageError(format!("write {}: {}", enc_path.display(), e))
                    })?;
                    debug!(
                        "save_face: encrypted embedding written to {}",
                        enc_path.display()
                    );
                }
                Err(e) => {
                    warn!(
                        "save_face: embedding encryption failed ({}), falling back to plaintext",
                        e
                    );
                    self.write_plaintext_embedding(&user_dir, record, embedding)?;
                }
            },
            Err(e) => {
                debug!(
                    "save_face: no TPM/key available ({}), storing embedding in plaintext",
                    e
                );
                self.write_plaintext_embedding(&user_dir, record, embedding)?;
            }
        }

        info!(
            "Face saved: user_id={}, face_id={} ({})",
            record.user_id,
            record.face_id,
            metadata_path.display(),
        );

        Ok(())
    }

    /// Plaintext embedding write — the pre-#152 behavior, kept as the
    /// fallback when no TPM-sealed key is available.
    fn write_plaintext_embedding(
        &self,
        user_dir: &Path,
        record: &FaceRecord,
        embedding: &Embedding,
    ) -> Result<(), DaemonError> {
        let embedding_path = face_path(user_dir, &record.face_id, ".embedding.json")?;
        let embedding_json =
            serde_json::to_string_pretty(embedding).map_err(DaemonError::JsonError)?;
        write_owner_only_file(&embedding_path, &embedding_json).map_err(|e| {
            warn!(
                "save_face: failed to write embedding to {}: {}",
                embedding_path.display(),
                e
            );
            DaemonError::StorageError(format!("write {}: {}", embedding_path.display(), e))
        })?;
        debug!(
            "save_face: embedding written to {}",
            embedding_path.display()
        );
        Ok(())
    }

    /// Path to this tree's own TPM-sealed embedding-encryption key —
    /// `base_path/secrets/embedding-key.tpm-sealed`. Ensures (and hardens)
    /// the containing directory exists; the key file itself may not exist
    /// yet (created lazily by `embedding_cipher::load_or_create_key`).
    fn own_embedding_key_path(&self) -> Result<PathBuf, DaemonError> {
        let dir = self.base_path.join("secrets");
        std::fs::create_dir_all(&dir)
            .map_err(|e| DaemonError::StorageError(format!("secrets dir creation: {}", e)))?;
        harden_dir(&dir)?;
        Ok(dir.join("embedding-key.tpm-sealed"))
    }

    /// Load an embedding by face_id
    pub fn load_face_embedding(
        &self,
        user_id: u32,
        face_id: &str,
    ) -> Result<Embedding, DaemonError> {
        let user_dir = self.user_dir(user_id)?;
        self.load_face_embedding_from_dir(&user_dir, user_id, face_id, None)
    }

    /// Load several embeddings for the same user in one call — resolves
    /// (and `canonicalize`s) `user_dir`, and unseals the decryption key,
    /// once up front instead of once per `face_id`, unlike calling
    /// `load_face_embedding` in a loop (a real TPM round trip is not cheap).
    /// Used by `verify_with_storage`.
    ///
    /// Skips (with a `warn!`) any face that can't be read or decrypted
    /// instead of failing the whole batch — a `RootRelayStore` face root
    /// hasn't received a synced copy of yet, or an `OwnPrincipal` face whose
    /// key currently can't be unsealed (e.g. `tpm2-abrmd` briefly down),
    /// just ends up absent from the result. `verify_with_storage`'s
    /// existing "no faces to compare against" path already degrades that to
    /// a plain non-match, so this never turns a partial availability
    /// problem into a hard failure of the whole verify attempt.
    pub fn load_face_embeddings(
        &self,
        user_id: u32,
        face_ids: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<std::collections::HashMap<String, Embedding>, DaemonError> {
        let user_dir = self.user_dir(user_id)?;
        let is_root = unsafe { libc::getuid() } == 0;
        let cached_key: Option<[u8; 32]> = match &self.embedding_source {
            EmbeddingSource::OwnPrincipal => self
                .own_embedding_key_path()
                .ok()
                .and_then(|path| embedding_cipher::load_key(&path, is_root).ok()),
            EmbeddingSource::RootRelayStore { uid } => {
                embedding_cipher::load_key(&root_embedding_key_path(*uid), true).ok()
            }
        };

        let mut out = std::collections::HashMap::new();
        for face_id in face_ids {
            let face_id = face_id.as_ref();
            match self.load_face_embedding_from_dir(
                &user_dir,
                user_id,
                face_id,
                cached_key.as_ref(),
            ) {
                Ok(embedding) => {
                    out.insert(face_id.to_string(), embedding);
                }
                Err(e) => warn!(
                    "load_face_embeddings: skipping user_id={} face_id={}: {}",
                    user_id, face_id, e
                ),
            }
        }
        Ok(out)
    }

    fn load_face_embedding_from_dir(
        &self,
        user_dir: &Path,
        user_id: u32,
        face_id: &str,
        cached_key: Option<&[u8; 32]>,
    ) -> Result<Embedding, DaemonError> {
        match &self.embedding_source {
            EmbeddingSource::RootRelayStore { uid } => {
                let uid = *uid;
                let key = match cached_key {
                    Some(k) => *k,
                    None => embedding_cipher::load_key(&root_embedding_key_path(uid), true)
                        .map_err(|e| {
                            DaemonError::StorageError(format!("root embedding key: {}", e))
                        })?,
                };
                let path = face_path(&root_embeddings_dir(uid), face_id, ".embedding.enc")?;
                let ciphertext = std::fs::read(&path).map_err(|e| {
                    DaemonError::StorageError(format!("root embedding read: {}", e))
                })?;
                embedding_cipher::decrypt_embedding(&key, &ciphertext, uid, face_id).map_err(|e| {
                    DaemonError::StorageError(format!("root embedding decrypt: {}", e))
                })
            }
            EmbeddingSource::OwnPrincipal => {
                self.load_own_embedding_from_dir(user_dir, user_id, face_id, cached_key)
            }
        }
    }

    /// `OwnPrincipal` read path: prefers `<face_id>.embedding.enc`, falling
    /// back to the legacy plaintext `<face_id>.embedding.json` and
    /// opportunistically migrating it in place — best-effort, never fails
    /// this read even if migration itself fails (e.g. no TPM/key currently
    /// available; the next read tries again).
    fn load_own_embedding_from_dir(
        &self,
        user_dir: &Path,
        user_id: u32,
        face_id: &str,
        cached_key: Option<&[u8; 32]>,
    ) -> Result<Embedding, DaemonError> {
        let enc_path = face_path(user_dir, face_id, ".embedding.enc")?;
        let legacy_path = face_path(user_dir, face_id, ".embedding.json")?;
        let is_root = unsafe { libc::getuid() } == 0;

        if enc_path.exists() {
            let ciphertext = std::fs::read(&enc_path)
                .map_err(|e| DaemonError::StorageError(format!("Embedding read: {}", e)))?;
            let key = match cached_key {
                Some(k) => *k,
                None => {
                    let key_path = self.own_embedding_key_path()?;
                    embedding_cipher::load_key(&key_path, is_root).map_err(|e| {
                        DaemonError::StorageError(format!("embedding key unavailable: {}", e))
                    })?
                }
            };
            let embedding =
                embedding_cipher::decrypt_embedding(&key, &ciphertext, user_id, face_id)
                    .map_err(|e| DaemonError::StorageError(format!("embedding decrypt: {}", e)))?;
            // Self-heal: an interrupted earlier migration can leave both an
            // .enc and a stale .json for the same face — .enc always wins
            // on read, so an orphaned plaintext copy is just dead weight,
            // removed opportunistically once we know it's safe to.
            if legacy_path.exists() {
                let _ = std::fs::remove_file(&legacy_path);
            }
            return Ok(embedding);
        }

        let content = std::fs::read_to_string(&legacy_path)
            .map_err(|e| DaemonError::StorageError(format!("Embedding read: {}", e)))?;
        let embedding: Embedding =
            serde_json::from_str(&content).map_err(DaemonError::JsonError)?;

        // Lazy migration to encrypted storage — best-effort, never fails
        // this read.
        if let Ok(key_path) = self.own_embedding_key_path() {
            if let Ok(key) = embedding_cipher::load_or_create_key(&key_path, is_root) {
                if let Err(e) = migrate_plaintext_embedding(
                    &enc_path,
                    &legacy_path,
                    &key,
                    user_id,
                    face_id,
                    &embedding,
                ) {
                    warn!(
                        "embedding migration failed for user_id={} face_id={}: {}",
                        user_id, face_id, e
                    );
                }
            }
        }

        Ok(embedding)
    }

    /// List all faces of a user
    pub fn list_user_faces(&self, user_id: u32) -> Result<Vec<FaceRecord>, DaemonError> {
        let user_dir = self.user_dir(user_id)?;

        if !user_dir.exists() {
            return Ok(Vec::new());
        }

        let mut faces = Vec::new();

        for entry in std::fs::read_dir(&user_dir)
            .map_err(|e| DaemonError::StorageError(format!("User dir read: {}", e)))?
        {
            let entry =
                entry.map_err(|e| DaemonError::StorageError(format!("Dir entry: {}", e)))?;
            let path = entry.path();

            // Look for .meta.json files
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".meta.json"))
                .unwrap_or(false)
            {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| DaemonError::StorageError(format!("Meta read: {}", e)))?;

                let record: FaceRecord =
                    serde_json::from_str(&content).map_err(DaemonError::JsonError)?;

                faces.push(record);
            }
        }

        Ok(faces)
    }

    /// Delete a face
    pub fn delete_face(&self, user_id: u32, face_id: &str) -> Result<(), DaemonError> {
        // face_id is attacker-controlled (it comes straight from the D-Bus
        // DeleteFace request body) and, unlike user_id (a plain u32), needs
        // validating before being joined onto a path — user_dir() guards
        // against user_id escaping base_path, but a face_id like
        // "../../../../etc/cron.d/x" would still escape *this* user's own
        // directory once appended below. face_path() does that check.
        let user_dir = self.user_dir(user_id)?;

        let meta_path = face_path(&user_dir, face_id, ".meta.json")?;
        let emb_path = face_path(&user_dir, face_id, ".embedding.json")?;
        let enc_path = face_path(&user_dir, face_id, ".embedding.enc")?;

        if meta_path.exists() {
            std::fs::remove_file(&meta_path)
                .map_err(|e| DaemonError::StorageError(format!("Meta deletion: {}", e)))?;
        }

        if emb_path.exists() {
            std::fs::remove_file(&emb_path)
                .map_err(|e| DaemonError::StorageError(format!("Embedding deletion: {}", e)))?;
        }

        if enc_path.exists() {
            std::fs::remove_file(&enc_path).map_err(|e| {
                DaemonError::StorageError(format!("Encrypted embedding deletion: {}", e))
            })?;
        }

        debug!("Face deleted: user_id={}, face_id={}", user_id, face_id);

        Ok(())
    }

    /// Delete all faces of a user
    pub fn delete_all_faces(&self, user_id: u32) -> Result<(), DaemonError> {
        let user_dir = self.user_dir(user_id)?;

        if user_dir.exists() {
            std::fs::remove_dir_all(&user_dir)
                .map_err(|e| DaemonError::StorageError(format!("User dir deletion: {}", e)))?;
        }

        debug!("All faces deleted for user_id={}", user_id);

        Ok(())
    }

    /// Get the user's directory
    fn user_dir(&self, user_id: u32) -> Result<PathBuf, DaemonError> {
        let user_dir = self.base_path.join(format!("users/{}", user_id));

        // Verify we don't escape base_path (security)
        // Use a simpler approach: check that the normalized path starts with base_path
        let normalized_user = user_dir.canonicalize().ok();
        let normalized_base = self.base_path.canonicalize().ok();

        match (normalized_user, normalized_base) {
            (Some(user), Some(base)) => {
                if !user.starts_with(&base) {
                    return Err(DaemonError::AccessDenied(format!(
                        "Path traversal attempt for user_id={}",
                        user_id
                    )));
                }
            }
            _ => {
                // If the path doesn't exist yet, do a simple check
                // Verify that "../" is not in the path
                let user_str = user_dir.to_string_lossy();
                if user_str.contains("..") {
                    return Err(DaemonError::AccessDenied(format!(
                        "Path traversal attempt for user_id={}",
                        user_id
                    )));
                }
            }
        }

        Ok(user_dir)
    }
}

/// Whether `face_id` is safe to join onto a filesystem path. Real face_ids
/// are always `face_<uid>_<timestamp>` (see `register_face`), so this is
/// deliberately narrow — alphanumeric, `_`, and `-` only, non-empty — rather
/// than trying to blocklist `/`/`..`/etc, which is easy to get wrong.
///
/// `pub(crate)`: also used by `pam_helper`'s embedding-relay handler to
/// validate a `face_id` that crossed a socket before it's ever joined onto
/// root's own relay-store path, even though the peer-uid check already
/// authenticates the *sender*.
pub(crate) fn is_safe_face_id(face_id: &str) -> bool {
    !face_id.is_empty()
        && face_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Joins `face_id` onto `user_dir` with the given filename suffix (e.g.
/// `.meta.json`), rejecting an unsafe `face_id` — the shared path-joining
/// layer every method that touches a face's files goes through, so the
/// `is_safe_face_id` check can't be missed by a future caller the way it
/// almost was here: only `delete_face` validated its (D-Bus-attacker-
/// controlled) `face_id` before this, while `save_face`/
/// `load_face_embedding_from_dir` built the identical kind of path with no
/// check of their own, safe only because their current callers happen to
/// always supply a machine-generated `face_id`.
fn face_path(user_dir: &Path, face_id: &str, suffix: &str) -> Result<PathBuf, DaemonError> {
    if !is_safe_face_id(face_id) {
        return Err(DaemonError::AccessDenied(format!(
            "Invalid face_id: {}",
            face_id
        )));
    }
    Ok(user_dir.join(format!("{}{}", face_id, suffix)))
}

/// Root-owned directory holding one embedding-key/embeddings tree per uid —
/// same `LINUX_HELLO_SECRETS_DIR` env convention as `secret_cache`'s own
/// `sealed_key_dir`, default `/var/lib/linux-hello/secrets`. Never readable
/// by the owning user, since only `hello-daemon-system` ever reads it.
fn root_secrets_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("LINUX_HELLO_SECRETS_DIR")
            .unwrap_or_else(|_| "/var/lib/linux-hello/secrets".to_string()),
    )
}

fn root_embeddings_dir(uid: u32) -> PathBuf {
    root_secrets_dir().join("embeddings").join(uid.to_string())
}

fn root_embedding_key_path(uid: u32) -> PathBuf {
    root_embeddings_dir(uid).join("key.tpm-sealed")
}

/// Pushes (upserts) root's own independently-sealed copy of one embedding —
/// called only from `pam_helper`'s embedding-relay socket handler, on
/// behalf of that uid's own `hello-daemon` (peer-uid-verified there).
/// Idempotent: always re-encrypts under root's current key, so the caller
/// never needs to check whether a copy already exists first.
pub fn push_root_embedding(record: &FaceRecord, embedding: &Embedding) -> Result<(), DaemonError> {
    let uid = record.user_id;
    let dir = root_embeddings_dir(uid);
    std::fs::create_dir_all(&dir)
        .map_err(|e| DaemonError::StorageError(format!("root embeddings dir creation: {}", e)))?;
    harden_dir(&dir)?;

    let key = embedding_cipher::load_or_create_key(&root_embedding_key_path(uid), true)
        .map_err(|e| DaemonError::StorageError(format!("root embedding key: {}", e)))?;
    let ciphertext = embedding_cipher::encrypt_embedding(&key, embedding, uid, &record.face_id)
        .map_err(|e| DaemonError::StorageError(format!("root embedding encrypt: {}", e)))?;
    let path = face_path(&dir, &record.face_id, ".embedding.enc")?;
    write_owner_only_file(&path, &ciphertext)
        .map_err(|e| DaemonError::StorageError(format!("write {}: {}", path.display(), e)))?;
    debug!(
        "push_root_embedding: stored root copy for uid={} face_id={}",
        uid, record.face_id
    );
    Ok(())
}

/// Deletes one face (`Some(face_id)`) or every face (`None`) from root's own
/// relay store for `uid` — called only from the embedding-relay handler.
/// A no-op, not an error, if there was nothing to delete (root never synced
/// this face/uid in the first place, e.g. it was enrolled and deleted again
/// before any sync happened).
pub fn delete_root_embedding(uid: u32, face_id: Option<&str>) -> Result<(), DaemonError> {
    match face_id {
        Some(face_id) => {
            let path = face_path(&root_embeddings_dir(uid), face_id, ".embedding.enc")?;
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| {
                    DaemonError::StorageError(format!("root embedding deletion: {}", e))
                })?;
            }
        }
        None => {
            let dir = root_embeddings_dir(uid);
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| {
                    DaemonError::StorageError(format!("root embeddings dir deletion: {}", e))
                })?;
            }
        }
    }
    Ok(())
}

/// Crash-safe plaintext→encrypted rewrite for one embedding file: encrypt to
/// a temp file, atomically rename it over the final `.enc` path, only then
/// remove the legacy plaintext — a crash between the rename and the
/// unlink leaves a harmless orphaned `.json` file that the next read's
/// self-heal step (see `load_own_embedding_from_dir`) cleans up, never a
/// partially-written or lost embedding.
fn migrate_plaintext_embedding(
    enc_path: &Path,
    legacy_path: &Path,
    key: &[u8; 32],
    user_id: u32,
    face_id: &str,
    embedding: &Embedding,
) -> Result<(), DaemonError> {
    let ciphertext = embedding_cipher::encrypt_embedding(key, embedding, user_id, face_id)
        .map_err(|e| DaemonError::StorageError(format!("embedding encrypt: {}", e)))?;
    let tmp_path = enc_path.with_extension("enc.tmp");
    write_owner_only_file(&tmp_path, &ciphertext)
        .map_err(|e| DaemonError::StorageError(format!("write {}: {}", tmp_path.display(), e)))?;
    std::fs::rename(&tmp_path, enc_path).map_err(|e| {
        DaemonError::StorageError(format!("rename to {}: {}", enc_path.display(), e))
    })?;
    std::fs::remove_file(legacy_path).map_err(|e| {
        DaemonError::StorageError(format!("remove {}: {}", legacy_path.display(), e))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_init() {
        let temp = TempDir::new().unwrap();
        let _ = FaceStorage::new(temp.path()).unwrap();

        assert!(temp.path().join("embeddings").exists());
    }

    #[test]
    fn test_save_and_load_face() {
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();

        let record = FaceRecord {
            face_id: "test_face_1".to_string(),
            user_id: 1000,
            quality_score: 0.95,
            registered_at: 0,
            context: "test".to_string(),
        };

        let embedding = Embedding {
            vector: vec![0.1, 0.2, 0.3],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        };

        storage.save_face(&record, &embedding).unwrap();

        let loaded = storage.load_face_embedding(1000, "test_face_1").unwrap();
        assert_eq!(loaded.vector.len(), 3);
    }

    #[test]
    fn test_list_faces() {
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();

        let record1 = FaceRecord {
            face_id: "face_1".to_string(),
            user_id: 1000,
            quality_score: 0.95,
            registered_at: 0,
            context: "test".to_string(),
        };

        let record2 = FaceRecord {
            face_id: "face_2".to_string(),
            user_id: 1000,
            quality_score: 0.92,
            registered_at: 0,
            context: "test".to_string(),
        };

        let embedding = Embedding {
            vector: vec![0.1, 0.2, 0.3],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        };

        storage.save_face(&record1, &embedding).unwrap();
        storage.save_face(&record2, &embedding).unwrap();

        let faces = storage.list_user_faces(1000).unwrap();
        assert_eq!(faces.len(), 2);
    }

    #[test]
    fn test_open_read_only_missing_dir_returns_none_and_creates_nothing() {
        // This is the property the SDDM system listener depends on: checking
        // an arbitrary, not-yet-authenticated user's storage must never
        // create directories in their home as a side effect.
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join("never-enrolled-user");

        let result = FaceStorage::open_read_only(&missing).unwrap();

        assert!(result.is_none());
        assert!(
            !missing.exists(),
            "open_read_only must not create the directory"
        );
    }

    #[test]
    fn test_open_read_only_existing_dir_can_list_faces() {
        let temp = TempDir::new().unwrap();
        // Set up with the side-effecting constructor once, as enrollment
        // would have already done.
        let storage = FaceStorage::new(temp.path()).unwrap();
        let record = FaceRecord {
            face_id: "face_1".to_string(),
            user_id: 1000,
            quality_score: 0.9,
            registered_at: 0,
            context: "sddm".to_string(),
        };
        let embedding = Embedding {
            vector: vec![0.1, 0.2],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.9,
            },
        };
        storage.save_face(&record, &embedding).unwrap();
        drop(storage);

        // Now re-open read-only, as the system listener would per-request.
        let reopened = FaceStorage::open_read_only(temp.path())
            .unwrap()
            .expect("directory exists, should return Some");
        let faces = reopened.list_user_faces(1000).unwrap();
        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].face_id, "face_1");
    }

    #[test]
    fn test_is_safe_face_id() {
        assert!(is_safe_face_id("face_1000_1735036800"));
        assert!(is_safe_face_id("face_1"));
        assert!(!is_safe_face_id(""));
        assert!(!is_safe_face_id("../../../etc/cron.d/x"));
        assert!(!is_safe_face_id("../x"));
        assert!(!is_safe_face_id("a/b"));
        assert!(!is_safe_face_id("a.b"));
        assert!(!is_safe_face_id("/etc/passwd"));
    }

    #[test]
    fn test_delete_face_still_works_for_a_legitimate_face_id() {
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();
        let record = FaceRecord {
            face_id: "face_1000_1735036800".to_string(),
            user_id: 1000,
            quality_score: 0.95,
            registered_at: 0,
            context: "test".to_string(),
        };
        let embedding = Embedding {
            vector: vec![0.1, 0.2, 0.3],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        };
        storage.save_face(&record, &embedding).unwrap();

        storage
            .delete_face(1000, "face_1000_1735036800")
            .expect("a real, well-formed face_id must still delete successfully");

        assert!(storage
            .load_face_embedding(1000, "face_1000_1735036800")
            .is_err());
    }

    #[test]
    fn test_delete_face_rejects_a_path_traversal_face_id() {
        // Regression test for the path-traversal fix: a crafted face_id
        // must not be able to reach a file outside this user's own
        // directory, even though the file it targets genuinely exists and
        // is genuinely reachable via that many "../" segments.
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();

        // A file outside any user's directory that a traversal could target.
        let canary = temp.path().join("canary.meta.json");
        std::fs::write(&canary, "should not be deleted").unwrap();

        // "users/1000/" is 2 levels deep under temp.path(), so "../../.."
        // reaches temp.path() itself, landing on "canary" once the code
        // appends ".meta.json".
        let result = storage.delete_face(1000, "../../../canary");

        assert!(result.is_err(), "traversal face_id must be rejected");
        assert!(
            canary.exists(),
            "the file outside the user dir must survive"
        );
    }

    #[test]
    fn test_save_face_rejects_a_path_traversal_face_id() {
        // face_path() is the shared path-joining layer save_face/
        // load_face_embedding/delete_face all now go through — this checks
        // it actually gates save_face too, not just delete_face (the one
        // call site that had its own bespoke is_safe_face_id check before
        // face_path existed).
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();

        let record = FaceRecord {
            face_id: "../../../etc/cron.d/evil".to_string(),
            user_id: 1000,
            quality_score: 0.95,
            registered_at: 0,
            context: "test".to_string(),
        };
        let embedding = Embedding {
            vector: vec![0.1, 0.2, 0.3],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        };

        assert!(storage.save_face(&record, &embedding).is_err());
        assert!(storage
            .load_face_embedding(1000, "../../../etc/cron.d/evil")
            .is_err());
    }

    /// Regression test: biometric data must never be left readable by other
    /// local users, regardless of the process umask. Without the explicit
    /// `harden_dir`/`write_owner_only_file` calls, these would end up at
    /// whatever the umask allows (0755/0644 under systemd's default 0022).
    #[test]
    fn test_save_face_restricts_directory_and_file_permissions() {
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();

        let record = FaceRecord {
            face_id: "face_1000_1735036800".to_string(),
            user_id: 1000,
            quality_score: 0.95,
            registered_at: 0,
            context: "test".to_string(),
        };
        let embedding = Embedding {
            vector: vec![0.1, 0.2, 0.3],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        };
        storage.save_face(&record, &embedding).unwrap();

        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;

        assert_eq!(mode(temp.path()), 0o700, "base storage dir");
        assert_eq!(
            mode(&temp.path().join("embeddings")),
            0o700,
            "embeddings dir"
        );

        let user_dir = temp.path().join("users/1000");
        assert_eq!(mode(&user_dir), 0o700, "per-user dir");
        assert_eq!(
            mode(&user_dir.join("face_1000_1735036800.meta.json")),
            0o600,
            "metadata file"
        );
        // Whichever form the embedding was written in (plaintext, if no TPM
        // is available in this test environment, or encrypted otherwise —
        // see save_face_encrypts_the_embedding_when_a_tpm_is_available for
        // that path specifically) must still be 0600.
        let embedding_file = [".embedding.json", ".embedding.enc"]
            .into_iter()
            .map(|suffix| user_dir.join(format!("face_1000_1735036800{}", suffix)))
            .find(|p| p.exists())
            .expect("save_face must have written an embedding file in one form or the other");
        assert_eq!(mode(&embedding_file), 0o600, "embedding file");
    }

    fn tpm_available_for_tests() -> bool {
        std::env::var("LINUX_HELLO_TPM_TCTI").is_ok()
    }

    fn sample_record(face_id: &str, user_id: u32) -> FaceRecord {
        FaceRecord {
            face_id: face_id.to_string(),
            user_id,
            quality_score: 0.95,
            registered_at: 0,
            context: "test".to_string(),
        }
    }

    fn sample_embedding_vec(v: Vec<f32>) -> Embedding {
        Embedding {
            vector: v,
            metadata: hello_face_core::EmbeddingMetadata {
                model: "test".to_string(),
                model_version: "0.1.0".to_string(),
                extracted_at: 0,
                quality_score: 0.95,
            },
        }
    }

    #[test]
    fn save_face_encrypts_the_embedding_when_a_tpm_is_available() {
        if !tpm_available_for_tests() {
            eprintln!("skipping: set LINUX_HELLO_TPM_TCTI to run against a real/simulated TPM");
            return;
        }
        let _guard = crate::secret_cache::ENV_VAR_GUARD.blocking_lock();
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();
        let record = sample_record("face_1000_1", 1000);
        let embedding = sample_embedding_vec(vec![0.1, 0.2, 0.3]);

        storage.save_face(&record, &embedding).unwrap();

        let user_dir = temp.path().join("users/1000");
        assert!(
            user_dir.join("face_1000_1.embedding.enc").exists(),
            "expected an encrypted embedding file"
        );
        assert!(
            !user_dir.join("face_1000_1.embedding.json").exists(),
            "must not also leave a plaintext copy"
        );

        let loaded = storage.load_face_embedding(1000, "face_1000_1").unwrap();
        assert_eq!(loaded.vector, embedding.vector);
    }

    #[test]
    fn legacy_plaintext_embedding_is_migrated_to_encrypted_on_read() {
        if !tpm_available_for_tests() {
            eprintln!("skipping: set LINUX_HELLO_TPM_TCTI to run against a real/simulated TPM");
            return;
        }
        let _guard = crate::secret_cache::ENV_VAR_GUARD.blocking_lock();
        let temp = TempDir::new().unwrap();
        let storage = FaceStorage::new(temp.path()).unwrap();
        let record = sample_record("face_1000_2", 1000);
        let embedding = sample_embedding_vec(vec![0.4, 0.5, 0.6]);

        // Write metadata + a *plaintext* embedding directly, bypassing
        // save_face — simulating a face enrolled before this feature
        // existed.
        let user_dir = temp.path().join("users/1000");
        std::fs::create_dir_all(&user_dir).unwrap();
        std::fs::write(
            user_dir.join("face_1000_2.meta.json"),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();
        std::fs::write(
            user_dir.join("face_1000_2.embedding.json"),
            serde_json::to_string(&embedding).unwrap(),
        )
        .unwrap();

        let loaded = storage.load_face_embedding(1000, "face_1000_2").unwrap();
        assert_eq!(loaded.vector, embedding.vector);

        assert!(
            user_dir.join("face_1000_2.embedding.enc").exists(),
            "read should have migrated the embedding to encrypted storage"
        );
        assert!(
            !user_dir.join("face_1000_2.embedding.json").exists(),
            "the legacy plaintext copy should be gone after a successful migration"
        );

        // A second read must transparently use the now-encrypted copy.
        let loaded_again = storage.load_face_embedding(1000, "face_1000_2").unwrap();
        assert_eq!(loaded_again.vector, embedding.vector);
    }

    #[test]
    fn push_and_delete_root_embedding_round_trip() {
        if !tpm_available_for_tests() {
            eprintln!("skipping: set LINUX_HELLO_TPM_TCTI to run against a real/simulated TPM");
            return;
        }
        let _guard = crate::secret_cache::ENV_VAR_GUARD.blocking_lock();
        let secrets_dir = TempDir::new().unwrap();
        std::env::set_var("LINUX_HELLO_SECRETS_DIR", secrets_dir.path());

        let record = sample_record("face_2000_1", 2000);
        let embedding = sample_embedding_vec(vec![0.7, 0.8, 0.9]);
        push_root_embedding(&record, &embedding).unwrap();

        // A RootRelayStore-mode FaceStorage never reads embeddings from the
        // "target home" it's opened against — an empty scratch dir with
        // just the metadata file is enough to prove that.
        let home = TempDir::new().unwrap();
        let user_dir = home.path().join("users/2000");
        std::fs::create_dir_all(&user_dir).unwrap();
        std::fs::write(
            user_dir.join("face_2000_1.meta.json"),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();

        let storage = FaceStorage::open_read_only_for_system_verify(home.path(), 2000)
            .unwrap()
            .expect("home dir exists");
        let loaded = storage.load_face_embeddings(2000, ["face_2000_1"]).unwrap();
        assert_eq!(
            loaded.get("face_2000_1").map(|e| &e.vector),
            Some(&embedding.vector)
        );

        delete_root_embedding(2000, Some("face_2000_1")).unwrap();
        let loaded_after_delete = storage.load_face_embeddings(2000, ["face_2000_1"]).unwrap();
        assert!(
            loaded_after_delete.is_empty(),
            "deleted root copy must no longer be returned"
        );

        std::env::remove_var("LINUX_HELLO_SECRETS_DIR");
    }

    #[test]
    fn root_relay_store_skips_a_face_with_no_synced_root_copy() {
        if !tpm_available_for_tests() {
            eprintln!("skipping: set LINUX_HELLO_TPM_TCTI to run against a real/simulated TPM");
            return;
        }
        let _guard = crate::secret_cache::ENV_VAR_GUARD.blocking_lock();
        let secrets_dir = TempDir::new().unwrap();
        std::env::set_var("LINUX_HELLO_SECRETS_DIR", secrets_dir.path());

        let home = TempDir::new().unwrap();
        let user_dir = home.path().join("users/3000");
        std::fs::create_dir_all(&user_dir).unwrap();
        let record = sample_record("face_3000_never_synced", 3000);
        std::fs::write(
            user_dir.join("face_3000_never_synced.meta.json"),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();

        let storage = FaceStorage::open_read_only_for_system_verify(home.path(), 3000)
            .unwrap()
            .expect("home dir exists");
        // Root never received a pushed copy for this face — must degrade to
        // "not present" rather than erroring the whole batch.
        let loaded = storage
            .load_face_embeddings(3000, ["face_3000_never_synced"])
            .unwrap();
        assert!(loaded.is_empty());

        std::env::remove_var("LINUX_HELLO_SECRETS_DIR");
    }
}
