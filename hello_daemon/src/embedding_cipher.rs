//! Encrypts face embeddings at rest with a TPM-sealed AES-256-GCM key —
//! see https://github.com/Aarklendoia/linux-hello/issues/152 and
//! `docs/PAM_MODULE.md` for the full design.
//!
//! # Two independent copies, not one shared secret
//!
//! Face embeddings are read by two different security principals: each
//! unprivileged per-user `hello-daemon` (for `sudo`/screenlock/its own
//! enroll-verify-list-delete), and root's `hello-daemon-system` (for
//! `context=sddm` only). Rather than one TPM secret both principals can
//! unseal (a `TPM2_PolicyOR` over two authorization branches — materially
//! more complex to provision correctly per enrolled user), each principal
//! seals and owns its **own independent key**, protecting its **own
//! independent copy** of every embedding. A compromise of one principal
//! never yields the other's key material. See [`crate::storage`]'s
//! `EmbeddingSource` for how the two copies are kept in sync.
//!
//! # Which TCTI, and why
//!
//! Root talks to the TPM directly (`TctiNameConf::Device`, same as
//! [`crate::secret_cache`] — root already has `/dev/tpmrm0` access). An
//! unprivileged per-user daemon instead goes through **`tpm2-abrmd`**
//! (`TctiNameConf::Tabrmd`, default system bus) rather than being added to
//! the `tss` group — an access-broker session-isolates TPM handles per D-Bus
//! connection, so granting ordinary users access to it is the intended
//! usage, not a broadening of trust (this project ships its own D-Bus policy
//! fragment granting that, since `tpm2-abrmd`'s own stock policy only covers
//! `root`/`tss`). `LINUX_HELLO_TPM_TCTI` always overrides both, same
//! convention `secret_cache` uses, for tests to point at `swtpm` directly or
//! at a locally-run `tpm2-abrmd`.
//!
//! # Policy: boot-integrity PCRs only, no liveness PCR
//!
//! Unlike [`crate::secret_cache`]'s password cache, an embedding must be
//! decryptable on **every** verify attempt — matched or not, since the
//! plaintext is needed *to determine* whether it's a match. There is no
//! "confirmed match" event available beforehand to gate release on the way
//! the password cache's liveness PCR does, so inventing one would be
//! circular. The policy here binds only to the same boot-integrity PCRs
//! 7/8/9 `secret_cache` also uses (see [`crate::tpm_seal::BOOT_PCR_SLOTS`]),
//! protecting exactly the threat this feature targets: offline disk theft
//! (reading the drive on different, or no, hardware). It does not — and
//! isn't meant to — defend against a live compromise of either principal,
//! whose blast radius is already whatever files that principal can read
//! today.
//!
//! One key protects every embedding under one (principal, storage-tree): a
//! per-user daemon only ever has its own uid in play, so this is really one
//! key per running daemon's storage; root's side is genuinely one key per
//! target uid, since one root process serves every enrolled user.

use std::convert::TryFrom;
use std::path::Path;

use aes_gcm::aead::{array::Array, Aead, KeyInit, Nonce, Payload};
use aes_gcm::Aes256Gcm;
use thiserror::Error;
use tss_esapi::{structures::SensitiveData, tcti_ldr::TctiNameConf};

use crate::tpm_seal::{self, TpmSealError};

#[derive(Debug, Error)]
pub enum EmbeddingCipherError {
    #[error("TPM error: {0}")]
    Tpm(#[from] tss_esapi::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("encryption error: {0}")]
    Crypto(String),
    #[error("no TPM/key available")]
    NoTpm,
}

impl From<TpmSealError> for EmbeddingCipherError {
    fn from(e: TpmSealError) -> Self {
        match e {
            TpmSealError::Tpm(e) => EmbeddingCipherError::Tpm(e),
            TpmSealError::Io(e) => EmbeddingCipherError::Io(e),
            TpmSealError::Crypto(s) => EmbeddingCipherError::Crypto(s),
        }
    }
}

/// Serializes and appends `sealed_key_path`'s creation with a process-wide
/// lock — without it, two concurrent first-time callers (e.g. the per-user
/// daemon's startup sync task racing a simultaneous enrollment) could both
/// observe "no key yet" and each seal a *different* random key, the second
/// write silently orphaning every embedding already encrypted under the
/// first. Reads never need this lock (they don't race each other); only the
/// check-then-create critical section does. A plain `std::sync::Mutex` is
/// fine here since every caller of `load_or_create_key` is itself blocking
/// (real TPM I/O) and always invoked via `spawn_blocking` — nothing ever
/// awaits while holding this.
static CREATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn tcti_conf(is_root: bool) -> Result<TctiNameConf, EmbeddingCipherError> {
    if let Ok(spec) = std::env::var("LINUX_HELLO_TPM_TCTI") {
        return spec.parse().map_err(|_| EmbeddingCipherError::NoTpm);
    }
    if is_root {
        Ok(TctiNameConf::Device(Default::default()))
    } else {
        Ok(TctiNameConf::Tabrmd(Default::default()))
    }
}

fn open_context(is_root: bool) -> Result<tss_esapi::Context, EmbeddingCipherError> {
    Ok(tpm_seal::open_context(tcti_conf(is_root)?)?)
}

/// Cheap-ish presence probe (one real TPM round trip: open a context, read
/// the boot-integrity PCRs) — used to decide fresh-enroll vs. plaintext
/// fallback without a hard error, and to answer the GUI's "is embedding
/// encryption active" status query (see `dbus::embedding_encryption_info`).
/// No liveness dance to probe here, unlike `secret_cache::probe_tpm_capability`.
pub fn probe(is_root: bool) -> bool {
    let mut ctx = match open_context(is_root) {
        Ok(ctx) => ctx,
        Err(_) => return false,
    };
    tpm_seal::pcr_selection_for(&tpm_seal::BOOT_PCR_SLOTS)
        .and_then(|sel| tpm_seal::pcr_digest(&mut ctx, &tpm_seal::BOOT_PCR_SLOTS, &sel))
        .is_ok()
}

fn create_and_seal_key(
    sealed_key_path: &Path,
    is_root: bool,
) -> Result<[u8; 32], EmbeddingCipherError> {
    let mut ctx = open_context(is_root)?;
    let key_bytes = tpm_seal::random_bytes::<32>()?;
    let selection = tpm_seal::pcr_selection_for(&tpm_seal::BOOT_PCR_SLOTS)?;
    let pcr_digest_now = tpm_seal::pcr_digest(&mut ctx, &tpm_seal::BOOT_PCR_SLOTS, &selection)?;
    let policy_digest = tpm_seal::trial_pcr_policy_digest(&mut ctx, pcr_digest_now, selection)?;
    let sensitive = SensitiveData::try_from(key_bytes.to_vec())
        .map_err(|e| EmbeddingCipherError::Crypto(e.to_string()))?;
    let (public, private) = tpm_seal::seal_sensitive_data(&mut ctx, sensitive, policy_digest)?;
    tpm_seal::write_sealed_blob(sealed_key_path, &public, &private)?;
    Ok(key_bytes)
}

/// Unseals the AES key already sealed at `sealed_key_path`. `Err(NoTpm)` if
/// no key has been sealed there yet — never creates one (use
/// [`load_or_create_key`] for that); callers on a pure decrypt path should
/// treat that, like any other error here, as "fall back to plaintext for
/// this face" rather than a hard failure.
///
/// Blocking (real TPM I/O) — callers on an async runtime must wrap this in
/// `tokio::task::spawn_blocking`.
pub fn load_key(sealed_key_path: &Path, is_root: bool) -> Result<[u8; 32], EmbeddingCipherError> {
    if !sealed_key_path.exists() {
        return Err(EmbeddingCipherError::NoTpm);
    }
    let mut ctx = open_context(is_root)?;
    let (public, private) = tpm_seal::read_sealed_blob(sealed_key_path)?;
    let selection = tpm_seal::pcr_selection_for(&tpm_seal::BOOT_PCR_SLOTS)?;
    let pcr_digest_now = tpm_seal::pcr_digest(&mut ctx, &tpm_seal::BOOT_PCR_SLOTS, &selection)?;
    let sensitive =
        tpm_seal::unseal_with_pcr_policy(&mut ctx, public, private, pcr_digest_now, selection)?;
    let bytes = sensitive.value();
    if bytes.len() != 32 {
        return Err(EmbeddingCipherError::Crypto(
            "unsealed key has the wrong length".to_string(),
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(bytes);
    Ok(key)
}

/// Unseals the key at `sealed_key_path`, sealing a fresh random one first if
/// none exists yet — the one entry point [`crate::storage`] calls from its
/// write path. See [`CREATE_LOCK`] for why first-time creation is
/// serialized.
///
/// Blocking (real TPM I/O) — callers on an async runtime must wrap this in
/// `tokio::task::spawn_blocking`.
pub fn load_or_create_key(
    sealed_key_path: &Path,
    is_root: bool,
) -> Result<[u8; 32], EmbeddingCipherError> {
    let guard = CREATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if sealed_key_path.exists() {
        drop(guard);
        return load_key(sealed_key_path, is_root);
    }
    create_and_seal_key(sealed_key_path, is_root)
}

/// `(user_id, face_id)` bound as AEAD associated data — a ciphertext file
/// silently moved or relabeled to answer for a different face/user fails
/// the GCM tag check rather than decrypting into a plausible-looking wrong
/// embedding.
fn aad_for(user_id: u32, face_id: &str) -> Vec<u8> {
    format!("{}:{}", user_id, face_id).into_bytes()
}

/// Encrypts a serialized [`hello_face_core::Embedding`] with `key`. On-disk
/// format: `nonce(12 bytes) || AES-256-GCM(serde_json(embedding))`.
pub fn encrypt_embedding(
    key: &[u8; 32],
    embedding: &hello_face_core::Embedding,
    user_id: u32,
    face_id: &str,
) -> Result<Vec<u8>, EmbeddingCipherError> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| EmbeddingCipherError::Crypto(e.to_string()))?;
    let nonce_bytes = tpm_seal::random_bytes::<12>()?;
    let nonce: Nonce<Aes256Gcm> = Array::try_from(nonce_bytes.as_slice())
        .map_err(|_| EmbeddingCipherError::Crypto("bad nonce length".to_string()))?;
    let plaintext =
        serde_json::to_vec(embedding).map_err(|e| EmbeddingCipherError::Crypto(e.to_string()))?;
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: &plaintext,
                aad: &aad_for(user_id, face_id),
            },
        )
        .map_err(|e| EmbeddingCipherError::Crypto(e.to_string()))?;
    let mut on_disk = Vec::with_capacity(nonce.len() + ciphertext.len());
    on_disk.extend_from_slice(&nonce);
    on_disk.extend_from_slice(&ciphertext);
    Ok(on_disk)
}

/// Reverses [`encrypt_embedding`]. Fails (rather than silently returning a
/// wrong embedding) if `user_id`/`face_id` don't match what it was encrypted
/// for, or the ciphertext/tag don't verify.
pub fn decrypt_embedding(
    key: &[u8; 32],
    on_disk: &[u8],
    user_id: u32,
    face_id: &str,
) -> Result<hello_face_core::Embedding, EmbeddingCipherError> {
    if on_disk.len() < 12 {
        return Err(EmbeddingCipherError::Crypto(
            "encrypted embedding truncated".to_string(),
        ));
    }
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| EmbeddingCipherError::Crypto(e.to_string()))?;
    let (nonce_bytes, ciphertext) = on_disk.split_at(12);
    let nonce: Nonce<Aes256Gcm> = Array::try_from(nonce_bytes)
        .map_err(|_| EmbeddingCipherError::Crypto("bad nonce length".to_string()))?;
    let plaintext = cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad: &aad_for(user_id, face_id),
            },
        )
        .map_err(|e| EmbeddingCipherError::Crypto(e.to_string()))?;
    serde_json::from_slice(&plaintext).map_err(|e| EmbeddingCipherError::Crypto(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tpm_available_for_tests() -> bool {
        std::env::var("LINUX_HELLO_TPM_TCTI").is_ok()
    }

    fn sample_embedding() -> hello_face_core::Embedding {
        hello_face_core::Embedding {
            vector: vec![0.1, 0.2, 0.3, -0.4],
            metadata: hello_face_core::EmbeddingMetadata {
                model: "arcface_mobilenet".to_string(),
                model_version: "1.0".to_string(),
                extracted_at: 1_700_000_000,
                quality_score: 0.9,
            },
        }
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = [7u8; 32];
        let embedding = sample_embedding();
        let on_disk = encrypt_embedding(&key, &embedding, 1000, "face_1000_1").unwrap();
        let decrypted = decrypt_embedding(&key, &on_disk, 1000, "face_1000_1").unwrap();
        assert_eq!(decrypted.vector, embedding.vector);
    }

    #[test]
    fn decrypt_fails_when_aad_does_not_match() {
        let key = [7u8; 32];
        let embedding = sample_embedding();
        let on_disk = encrypt_embedding(&key, &embedding, 1000, "face_1000_1").unwrap();
        // Same ciphertext, wrong face_id — must not decrypt.
        assert!(decrypt_embedding(&key, &on_disk, 1000, "face_1000_2").is_err());
        // Same ciphertext, wrong user_id — must not decrypt.
        assert!(decrypt_embedding(&key, &on_disk, 1001, "face_1000_1").is_err());
    }

    #[test]
    fn decrypt_fails_with_wrong_key() {
        let embedding = sample_embedding();
        let on_disk = encrypt_embedding(&[7u8; 32], &embedding, 1000, "face_1000_1").unwrap();
        assert!(decrypt_embedding(&[8u8; 32], &on_disk, 1000, "face_1000_1").is_err());
    }

    #[test]
    fn load_key_reports_no_tpm_when_nothing_sealed_yet() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.tpm-sealed");
        assert!(matches!(
            load_key(&path, true),
            Err(EmbeddingCipherError::NoTpm)
        ));
    }

    #[test]
    fn seal_unseal_round_trip_against_a_real_or_simulated_tpm() {
        if !tpm_available_for_tests() {
            eprintln!("skipping: set LINUX_HELLO_TPM_TCTI to run against a real/simulated TPM");
            return;
        }
        let _guard = crate::secret_cache::ENV_VAR_GUARD.blocking_lock();
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("uid.tpm-sealed");

        let key = load_or_create_key(&key_path, true).unwrap();
        let key_again = load_key(&key_path, true).unwrap();
        assert_eq!(key, key_again);

        // A second load_or_create_key call on the same path must return the
        // *same* key, not seal a brand-new one over it.
        let key_third = load_or_create_key(&key_path, true).unwrap();
        assert_eq!(key, key_third);

        let embedding = sample_embedding();
        let on_disk = encrypt_embedding(&key, &embedding, 42, "face_42_1").unwrap();
        let decrypted = decrypt_embedding(&key, &on_disk, 42, "face_42_1").unwrap();
        assert_eq!(decrypted.vector, embedding.vector);
    }
}
