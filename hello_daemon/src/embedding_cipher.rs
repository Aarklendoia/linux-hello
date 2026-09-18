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
//! # Policy: Secure Boot state only, no liveness PCR
//!
//! Unlike [`crate::secret_cache`]'s password cache, an embedding must be
//! decryptable on **every** verify attempt — matched or not, since the
//! plaintext is needed *to determine* whether it's a match. There is no
//! "confirmed match" event available beforehand to gate release on the way
//! the password cache's liveness PCR does, so inventing one would be
//! circular. The policy here binds to [`crate::tpm_seal::STABLE_PCR_SLOTS`]
//! (Secure Boot policy state only — see its own doc for why this project
//! stopped also binding to the bootloader/kernel measurement, PCR8:
//! confirmed on real hardware to change across a plain reboot with no
//! actual kernel/bootloader update involved), protecting exactly the threat
//! this feature targets: offline disk theft (reading the drive on
//! different, or no, hardware) on a Secure-Boot-enabled machine. It does
//! not — and isn't meant to — defend against a live compromise of either
//! principal, whose blast radius is already whatever files that principal
//! can read today.
//!
//! A `POLICY_FAIL` from the TPM (boot measurements no longer match what was
//! sealed) is therefore treated as its own [`EmbeddingCipherError::PolicyFailure`]
//! variant, distinct from a transient TPM error — see its doc for why
//! callers must not treat the two the same way.
//!
//! One key protects every embedding under one (principal, storage-tree): a
//! per-user daemon only ever has its own uid in play, so this is really one
//! key per running daemon's storage; root's side is genuinely one key per
//! target uid, since one root process serves every enrolled user.

use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use aes_gcm::aead::{array::Array, Aead, KeyInit, Nonce, Payload};
use aes_gcm::Aes256Gcm;
use thiserror::Error;
use tracing::warn;
use tss_esapi::{structures::SensitiveData, tcti_ldr::TctiNameConf};

use crate::tpm_seal::{self, TpmSealError};

#[derive(Debug, Error)]
pub enum EmbeddingCipherError {
    #[error("TPM error: {0}")]
    Tpm(tss_esapi::Error),
    /// The TPM's `TPM2_PolicyPCR` check failed: PCR7 (Secure Boot policy
    /// state) no longer matches what it was when this key was sealed —
    /// Secure Boot was toggled, or the trusted certificate database
    /// changed. Unlike
    /// [`EmbeddingCipherError::Tpm`]'s other cases (e.g. `tpm2-abrmd`
    /// momentarily unreachable), this is **not transient** — the exact old
    /// PCR digest can never recur, so the sealed key can never be unsealed
    /// again, and every embedding it protects is permanently unreadable.
    /// Callers must surface this distinctly (loudly, and pointing at
    /// re-enrollment) rather than folding it into ordinary "couldn't read
    /// this face, try again" handling. See
    /// https://github.com/Aarklendoia/linux-hello/issues/160.
    #[error(
        "TPM policy check failed: boot measurements changed since this key was sealed; \
         the protected embedding(s) can no longer be decrypted and must be re-enrolled"
    )]
    PolicyFailure,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("encryption error: {0}")]
    Crypto(String),
    #[error("no TPM/key available")]
    NoTpm,
}

/// `true` if `err` is a `TPM2_RC_POLICY_FAIL` response — the TPM's own
/// signal that a `TPM2_PolicyPCR` check didn't match, as opposed to any
/// other TSS/transport-level failure.
fn is_policy_failure(err: &tss_esapi::Error) -> bool {
    use tss_esapi::constants::Tss2ResponseCodeKind;
    matches!(
        err,
        tss_esapi::Error::Tss2Error(rc) if rc.kind() == Some(Tss2ResponseCodeKind::PolicyFail)
    )
}

impl From<tss_esapi::Error> for EmbeddingCipherError {
    fn from(e: tss_esapi::Error) -> Self {
        if is_policy_failure(&e) {
            EmbeddingCipherError::PolicyFailure
        } else {
            EmbeddingCipherError::Tpm(e)
        }
    }
}

impl From<TpmSealError> for EmbeddingCipherError {
    fn from(e: TpmSealError) -> Self {
        match e {
            TpmSealError::Tpm(e) => EmbeddingCipherError::from(e),
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

/// A cached outcome of unsealing one `sealed_key_path` — either the key
/// itself, or a remembered, permanent [`EmbeddingCipherError::PolicyFailure`].
/// See [`key_cache`]'s doc for why caching the failure case matters just as
/// much as caching the success case.
#[derive(Clone, Copy)]
enum CachedKeyState {
    Key([u8; 32]),
    PolicyFailure,
}

/// In-memory cache of already-attempted unseals, keyed by sealed-key file
/// path.
///
/// A real hardware TPM's PCR-policy unseal is not cheap — measured ~9
/// *seconds* round-tripping through `tpm2-abrmd` on real fTPM hardware, not
/// hypothetical. PCR7 (Secure Boot policy state) doesn't change until the
/// next reboot, so the outcome is valid for this whole process's lifetime;
/// re-unsealing on every call was pure waste, not extra security — and a
/// real, measured one, since `verify_with_storage` calls into here at least
/// once per authentication attempt (sudo, screenlock, polkit, SDDM), so
/// every single face login was paying that multi-second cost. Keeping the
/// plaintext key resident here for the process's life is no weaker than the
/// status quo: any process already holding it can decrypt on demand anyway,
/// and this is about offline disk theft, not a live-process compromise (see
/// the module doc's "Policy" section).
///
/// Caching [`CachedKeyState::PolicyFailure`] specifically (issue #160,
/// confirmed the hard way on real hardware: without this, a single stale
/// sealed key made *every* verify attempt pay a fresh, doomed ~9s TPM
/// round-trip — twice per attempt, once from `load_face_embeddings`'s own
/// speculative fetch and once more from `load_own_embedding_from_dir`'s
/// opportunistic plaintext→encrypted migration attempt, stacking to ~18s of
/// pure waste before the camera even started) rests on the exact same
/// invariant the success case already relies on: the PCR state that caused
/// it cannot change before the next reboot, so a `POLICY_FAIL` now will
/// still be a `POLICY_FAIL` on the next call this process makes. Never
/// evicted except by process restart or [`load_or_create_key`] resealing a
/// fresh key over the stale one (which also refreshes this cache entry).
fn key_cache() -> &'static Mutex<HashMap<PathBuf, CachedKeyState>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedKeyState>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn tcti_conf(is_root: bool) -> Result<TctiNameConf, EmbeddingCipherError> {
    if let Ok(spec) = std::env::var("LINUX_HELLO_TPM_TCTI") {
        return spec.parse().map_err(|_| EmbeddingCipherError::NoTpm);
    }
    if is_root {
        Ok(tpm_seal::root_device_tcti())
    } else {
        Ok(TctiNameConf::Tabrmd(Default::default()))
    }
}

fn open_context(is_root: bool) -> Result<tss_esapi::Context, EmbeddingCipherError> {
    Ok(tpm_seal::open_context(tcti_conf(is_root)?)?)
}

/// Root reuses a persistent parent key (see
/// `tpm_seal::root_persistent_primary_key`) instead of paying a fresh
/// `TPM2_CreatePrimary` on every call — safe only for root, which never
/// exposes that persistent handle to another, less-trusted principal (root
/// talks to `/dev/tpmrm0` directly, never through `tpm2-abrmd`). The
/// unprivileged, `tpm2-abrmd`-brokered path stays ephemeral.
fn parent_for(is_root: bool) -> tpm_seal::Parent {
    if is_root {
        tpm_seal::Parent::RootPersistent
    } else {
        tpm_seal::Parent::Ephemeral
    }
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
    tpm_seal::pcr_selection_for(&tpm_seal::STABLE_PCR_SLOTS)
        .and_then(|sel| tpm_seal::pcr_digest(&mut ctx, &tpm_seal::STABLE_PCR_SLOTS, &sel))
        .is_ok()
}

fn create_and_seal_key(
    sealed_key_path: &Path,
    is_root: bool,
) -> Result<[u8; 32], EmbeddingCipherError> {
    let mut ctx = open_context(is_root)?;
    let key_bytes = tpm_seal::random_bytes::<32>()?;
    let selection = tpm_seal::pcr_selection_for(&tpm_seal::STABLE_PCR_SLOTS)?;
    let pcr_digest_now = tpm_seal::pcr_digest(&mut ctx, &tpm_seal::STABLE_PCR_SLOTS, &selection)?;
    let policy_digest = tpm_seal::trial_pcr_policy_digest(&mut ctx, pcr_digest_now, selection)?;
    let sensitive = SensitiveData::try_from(key_bytes.to_vec())
        .map_err(|e| EmbeddingCipherError::Crypto(e.to_string()))?;
    let (public, private) =
        tpm_seal::seal_sensitive_data(&mut ctx, sensitive, policy_digest, parent_for(is_root))?;
    tpm_seal::write_sealed_blob(sealed_key_path, &public, &private)?;
    key_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(
            sealed_key_path.to_path_buf(),
            CachedKeyState::Key(key_bytes),
        );
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
    if let Some(state) = key_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(sealed_key_path)
    {
        return match state {
            CachedKeyState::Key(key) => Ok(*key),
            CachedKeyState::PolicyFailure => Err(EmbeddingCipherError::PolicyFailure),
        };
    }
    if !sealed_key_path.exists() {
        return Err(EmbeddingCipherError::NoTpm);
    }
    let result = unseal_from_disk(sealed_key_path, is_root);
    match &result {
        Ok(key) => {
            key_cache()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(sealed_key_path.to_path_buf(), CachedKeyState::Key(*key));
        }
        // Permanent (won't change before the next reboot) — cache it so
        // every later caller this process's lifetime fails fast instead of
        // repeating the same doomed ~9s TPM round-trip (issue #160).
        Err(EmbeddingCipherError::PolicyFailure) => {
            key_cache()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(sealed_key_path.to_path_buf(), CachedKeyState::PolicyFailure);
        }
        // Anything else (e.g. tpm2-abrmd momentarily unreachable) is
        // presumed transient — deliberately not cached, so the next call
        // gets a fresh attempt.
        Err(_) => {}
    }
    result
}

/// The actual TPM round-trip `load_key` performs on a cache miss — pulled
/// out so both `load_key` and its caching wrapper stay readable.
fn unseal_from_disk(
    sealed_key_path: &Path,
    is_root: bool,
) -> Result<[u8; 32], EmbeddingCipherError> {
    let mut ctx = open_context(is_root)?;
    let (public, private) = tpm_seal::read_sealed_blob(sealed_key_path)?;
    let selection = tpm_seal::pcr_selection_for(&tpm_seal::STABLE_PCR_SLOTS)?;
    let pcr_digest_now = tpm_seal::pcr_digest(&mut ctx, &tpm_seal::STABLE_PCR_SLOTS, &selection)?;
    let sensitive = tpm_seal::unseal_with_pcr_policy(
        &mut ctx,
        public,
        private,
        pcr_digest_now,
        selection,
        parent_for(is_root),
    )?;
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
/// none exists yet, or if the existing one is permanently stale — the one
/// entry point [`crate::storage`] calls from its write path. See
/// [`CREATE_LOCK`] for why first-time creation is serialized.
///
/// A `PolicyFailure` here (issue #160: boot measurements changed since this
/// key was sealed) is treated the same as "no key yet" rather than a hard
/// error: every embedding the stale key protected is already unrecoverable
/// regardless (the exact old PCR digest can never recur), so there's
/// nothing left to lose by resealing fresh under the *current* boot state —
/// the alternative is getting stuck writing plaintext forever, since
/// nothing else would ever replace a sealed-but-broken file. Any other
/// error (e.g. `tpm2-abrmd` momentarily down) is presumed transient and
/// still propagated as-is, without touching the existing file.
///
/// Blocking (real TPM I/O) — callers on an async runtime must wrap this in
/// `tokio::task::spawn_blocking`.
pub fn load_or_create_key(
    sealed_key_path: &Path,
    is_root: bool,
) -> Result<[u8; 32], EmbeddingCipherError> {
    let _guard = CREATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if sealed_key_path.exists() {
        match load_key(sealed_key_path, is_root) {
            Ok(key) => return Ok(key),
            Err(EmbeddingCipherError::PolicyFailure) => {
                warn!(
                    "embedding_cipher: {} is permanently stale (boot measurements changed \
                     since it was sealed) — resealing a fresh key under the current boot state",
                    sealed_key_path.display()
                );
                key_cache()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(sealed_key_path);
                std::fs::remove_file(sealed_key_path)?;
            }
            Err(e) => return Err(e),
        }
    }
    create_and_seal_key(sealed_key_path, is_root)
    // `guard` (still held) drops here, after the reseal completes.
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

    /// No real TPM needed: `Tss2ResponseCode::from` is a pure response-code
    /// decode, so a `POLICY_FAIL` response can be constructed directly and
    /// fed through the same `From<tss_esapi::Error>` conversion the real
    /// unseal path uses.
    #[test]
    fn policy_fail_response_is_classified_as_policy_failure() {
        use tss_esapi::constants::tss::TPM2_RC_POLICY_FAIL;
        use tss_esapi::constants::Tss2ResponseCode;

        let err = tss_esapi::Error::Tss2Error(Tss2ResponseCode::from(TPM2_RC_POLICY_FAIL));
        assert!(matches!(
            EmbeddingCipherError::from(err),
            EmbeddingCipherError::PolicyFailure
        ));
    }

    /// A different TPM error (session memory exhausted, picked arbitrarily —
    /// any non-`POLICY_FAIL` code works) must stay a generic, presumed-
    /// transient `Tpm` error, not get misclassified as the permanent case.
    #[test]
    fn other_tpm_errors_are_not_classified_as_policy_failure() {
        use tss_esapi::constants::tss::TPM2_RC_SESSION_MEMORY;
        use tss_esapi::constants::Tss2ResponseCode;

        let err = tss_esapi::Error::Tss2Error(Tss2ResponseCode::from(TPM2_RC_SESSION_MEMORY));
        assert!(matches!(
            EmbeddingCipherError::from(err),
            EmbeddingCipherError::Tpm(_)
        ));
    }

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
    fn load_or_create_key_reseals_automatically_after_a_policy_failure() {
        if !tpm_available_for_tests() {
            eprintln!("skipping: set LINUX_HELLO_TPM_TCTI to run against a real/simulated TPM");
            return;
        }
        let _guard = crate::secret_cache::ENV_VAR_GUARD.blocking_lock();
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("uid.tpm-sealed");

        // Seal a key against a policy digest that live PCRs can never
        // satisfy — simulates a key sealed under a boot state that's since
        // changed (issue #160), without needing an actual reboot.
        let mut ctx = open_context(true).unwrap();
        let stale_key_bytes = tpm_seal::random_bytes::<32>().unwrap();
        let bogus_pcr_digest = tss_esapi::structures::Digest::try_from(vec![0xAB; 32]).unwrap();
        let selection = tpm_seal::pcr_selection_for(&tpm_seal::STABLE_PCR_SLOTS).unwrap();
        let policy_digest =
            tpm_seal::trial_pcr_policy_digest(&mut ctx, bogus_pcr_digest, selection).unwrap();
        let sensitive = SensitiveData::try_from(stale_key_bytes.to_vec()).unwrap();
        let (public, private) = tpm_seal::seal_sensitive_data(
            &mut ctx,
            sensitive,
            policy_digest,
            tpm_seal::Parent::RootPersistent,
        )
        .unwrap();
        tpm_seal::write_sealed_blob(&key_path, &public, &private).unwrap();

        // Confirm it's genuinely broken the way a stale post-reboot key
        // would be, and that a second attempt fails just as fast (the
        // negative cache), not with another ~9s TPM round-trip.
        assert!(matches!(
            load_key(&key_path, true),
            Err(EmbeddingCipherError::PolicyFailure)
        ));
        assert!(matches!(
            load_key(&key_path, true),
            Err(EmbeddingCipherError::PolicyFailure)
        ));

        // load_or_create_key must notice, reseal fresh under the CURRENT
        // (real) PCR state, and hand back a working, different key — not
        // propagate the failure or keep serving the stale one.
        let fresh_key = load_or_create_key(&key_path, true).unwrap();
        assert_ne!(fresh_key, stale_key_bytes);

        // And the fresh key must actually be usable now.
        let reloaded = load_key(&key_path, true).unwrap();
        assert_eq!(reloaded, fresh_key);
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
