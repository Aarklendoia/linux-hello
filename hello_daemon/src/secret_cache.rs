//! TPM-sealed cache of a user's real login password, released only after a
//! verified `context=sddm` face match so `pam_linux_hello` can call
//! `pam_set_item(PAM_AUTHTOK, ...)` and let `pam_kwallet5`/`pam_gnome_keyring`
//! auto-unlock exactly as if the password had been typed — see
//! `docs/PAM_MODULE.md`'s "Password caching / KWallet auto-unlock" section
//! and https://github.com/Aarklendoia/linux-hello/issues/149 for the full
//! design and its honestly-stated limits.
//!
//! # Why this only ever runs inside `hello-daemon-system`
//!
//! Only the root-owned `hello-daemon-system` process ever calls into this
//! module (via the cache socket for [`seal_and_store`] and from
//! [`crate::pam_helper::handle_system_pam_request`] for [`release_for_match`])
//! — the same trust boundary already used for `context=sddm` face
//! verification. The AES key that actually protects the cached password is
//! sealed *inside the TPM* (`fixedTPM`/`fixedParent`, never exportable), so
//! the on-disk sealed blob is meaningless without that exact chip.
//!
//! Low-level TPM plumbing (context/TCTI setup, the primary key, PCR-policy
//! digest computation, seal/unseal, blob marshalling) lives in
//! [`crate::tpm_seal`], shared with [`crate::embedding_cipher`] — see that
//! module's doc comment for why. This module only adds what's specific to
//! caching a *password* released on a *confirmed live match*: the liveness
//! PCR extend/reset dance below.
//!
//! # The two PCR roles
//!
//! The seal's TPM policy is a compound `TPM2_PolicyPCR` over two different
//! kinds of PCR, read together at both seal time and release time:
//!
//! 1. **Boot-integrity PCRs 7/8/9** (Secure Boot state, kernel, initrd) — if
//!    any of these change (an altered boot chain), the policy digest no
//!    longer matches and the seal becomes permanently unusable on that
//!    machine, exactly like `systemd-cryptenroll --tpm2-pcrs=7,8,9` for LUKS.
//! 2. **A dedicated "liveness" PCR, PCR 16** — the TCG-reserved
//!    resettable/application PCR confirmed extendable and resettable from
//!    locality 0 by `tss-esapi`'s own upstream integration test
//!    (`test_pcr_extend_reset_commands`, which picked PCR16 specifically
//!    because "it's the only one that is resettable and extendable from the
//!    locality in which we are running" on the reference stack this project
//!    also targets) — PCR23 was the original candidate but is not
//!    guaranteed resettable from locality 0 on every implementation, so
//!    PCR16 is used instead. [`release_for_match`] extends it with a fixed,
//!    uid-specific tag right after a verified match, then resets it back to
//!    baseline once it's done — the extend→unseal→reset cycle repeats on
//!    every subsequent login within the same boot.
//!
//! **Honest limitation, stated here rather than discovered later:** the
//! liveness tag folded into PCR16 depends only on public constants (the uid
//! and a fixed string), so this does not *cryptographically* prove a face
//! match happened — a compromised root process on a correctly-booted,
//! untampered machine could replay the same extend call itself. What this
//! design actually buys: the sealed key is unusable if the disk/backup is
//! stolen (offline attack, since the key never leaves the TPM in the first
//! place) and unusable if the boot chain was tampered with — not immunity to
//! a live root compromise, which remains equivalent to today's
//! `/etc/shadow` trust level.

use std::convert::TryFrom;
use std::path::PathBuf;

use aes_gcm::aead::{array::Array, Aead, KeyInit, Nonce};
use aes_gcm::Aes256Gcm;
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use tracing::{debug, info, warn};
use tss_esapi::{
    handles::PcrHandle,
    interface_types::algorithm::HashingAlgorithm,
    structures::{Digest, DigestValues, PcrSelectionListBuilder, PcrSlot, SensitiveData},
    tcti_ldr::TctiNameConf,
    Context,
};
use zeroize::Zeroizing;

use crate::security_util::write_owner_only_file;
use crate::tpm_seal::{self, TpmSealError};

/// The dedicated liveness PCR — see the module docs above for why PCR16 was
/// chosen over the originally-proposed PCR23.
const LIVENESS_PCR_HANDLE: PcrHandle = PcrHandle::Pcr16;
const LIVENESS_PCR_SLOT: PcrSlot = PcrSlot::Slot16;

#[derive(Debug, Error)]
pub enum SecretCacheError {
    #[error("TPM error: {0}")]
    Tpm(#[from] tss_esapi::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("encryption error: {0}")]
    Crypto(String),
    #[error("no TPM available")]
    NoTpm,
}

impl From<TpmSealError> for SecretCacheError {
    fn from(e: TpmSealError) -> Self {
        match e {
            TpmSealError::Tpm(e) => SecretCacheError::Tpm(e),
            TpmSealError::Io(e) => SecretCacheError::Io(e),
            TpmSealError::Crypto(s) => SecretCacheError::Crypto(s),
        }
    }
}

/// What [`probe_tpm_capability`] found on this machine — surfaced to the
/// `cache-password` CLI/GUI so a hardware limitation is disclosed at the one
/// moment the user makes the trade-off, not discovered later as a silent
/// failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TpmCapability {
    /// A TPM answered and the boot-integrity PCRs could be read.
    pub available: bool,
    /// Whether `TPM2_PCR_Reset` on the liveness PCR actually restored it to
    /// baseline — if `false`, cache release still works once per boot, then
    /// falls back to the normal KWallet prompt until reboot (see the module
    /// docs' PCR16 section).
    pub liveness_pcr_resettable: bool,
}

/// Root-owned directory holding one TPM-sealed blob per uid — never readable
/// by the owning user, since only `hello-daemon-system` (root) ever needs it.
fn sealed_key_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("LINUX_HELLO_SECRETS_DIR")
            .unwrap_or_else(|_| "/var/lib/linux-hello/secrets".to_string()),
    )
}

fn sealed_key_path(uid: u32) -> PathBuf {
    sealed_key_dir().join(format!("{}.tpm-sealed", uid))
}

/// Encrypted password blob, placed alongside the TPM-sealed key blob in the
/// same root-owned `/var/lib/linux-hello/secrets/` directory — not under the
/// user's own home directory tree, even though it's otherwise a per-user
/// file in the same convention as `hello_daemon::storage`'s paths. Only the
/// root `hello-daemon-system` process ever touches it (both `seal_and_store`
/// and `release_for_match` run there), so there's no reason for the owning
/// user's own account to have direct read access to a file that's useless
/// without the sealed key next to it anyway — and critically,
/// `hello-daemon-system.service` runs under `ProtectHome=read-only`
/// (it only ever needs *read* access to home directories, to check for
/// enrolled faces), so a home-directory path here would hit `EROFS` the
/// first time this ran for real — caught exactly that way on a real
/// packaged install, not in testing (tests use tmpfs-backed tempdirs with no
/// sandboxing).
fn encrypted_authtok_path(uid: u32) -> PathBuf {
    sealed_key_dir().join(format!("{}.authtok.enc", uid))
}

fn tcti_conf() -> Result<TctiNameConf, SecretCacheError> {
    if let Ok(spec) = std::env::var("LINUX_HELLO_TPM_TCTI") {
        return spec.parse().map_err(|_| SecretCacheError::NoTpm);
    }
    Ok(TctiNameConf::Device(Default::default()))
}

fn open_context() -> Result<Context, SecretCacheError> {
    Ok(tpm_seal::open_context(tcti_conf()?)?)
}

/// The two-role PCR slot list (boot-integrity + liveness), in the fixed
/// order [`tpm_seal::pcr_digest`] concatenates them in.
fn full_pcr_slots() -> Vec<PcrSlot> {
    let mut slots = tpm_seal::BOOT_PCR_SLOTS.to_vec();
    slots.push(LIVENESS_PCR_SLOT);
    slots
}

fn extend_liveness_pcr(ctx: &mut Context, uid: u32) -> Result<(), SecretCacheError> {
    let tag = format!("linux-hello:auth-ok:{}", uid);
    let event_digest = Digest::try_from(Sha256::digest(tag.as_bytes()).to_vec())?;
    let mut values = DigestValues::new();
    values.set(HashingAlgorithm::Sha256, event_digest);

    Ok(tpm_seal::with_pcr_auth_session(ctx, |ctx| {
        Ok(ctx.pcr_extend(LIVENESS_PCR_HANDLE, values)?)
    })?)
}

/// Resets the liveness PCR back to baseline. Returns `Ok(false)` (not an
/// error) if the reset call itself succeeded but the PCR didn't actually go
/// back to zero — some TPM firmwares silently ignore a locality-0 reset on
/// PCRs other than the one they've hardcoded — the caller degrades to
/// "release worked once this boot" rather than treating it as fatal.
fn reset_liveness_pcr(ctx: &mut Context) -> Result<bool, SecretCacheError> {
    tpm_seal::with_pcr_auth_session(ctx, |ctx| Ok(ctx.pcr_reset(LIVENESS_PCR_HANDLE)?))?;

    let selection = PcrSelectionListBuilder::new()
        .with_selection(HashingAlgorithm::Sha256, &[LIVENESS_PCR_SLOT])
        .build()?;
    let (_, read_selections, read_digests) = ctx.pcr_read(selection)?;
    let pcr_data = tss_esapi::abstraction::pcr::PcrData::create(&read_selections, &read_digests)?;
    let value = pcr_data
        .pcr_bank(HashingAlgorithm::Sha256)
        .and_then(|bank| bank.get_digest(LIVENESS_PCR_SLOT).cloned());
    Ok(matches!(value, Some(d) if d.value().iter().all(|b| *b == 0)))
}

/// Probes TPM presence and liveness-PCR reset reliability — called once at
/// `cache-password` time so a hardware limitation is disclosed up front
/// (see [`TpmCapability`]), not discovered later as a silent failure.
pub fn probe_tpm_capability() -> TpmCapability {
    let mut ctx = match open_context() {
        Ok(ctx) => ctx,
        Err(_) => {
            return TpmCapability {
                available: false,
                liveness_pcr_resettable: false,
            }
        }
    };
    let slots = full_pcr_slots();
    let readable = tpm_seal::pcr_selection_for(&slots)
        .and_then(|sel| tpm_seal::pcr_digest(&mut ctx, &slots, &sel))
        .is_ok();
    if !readable {
        return TpmCapability {
            available: false,
            liveness_pcr_resettable: false,
        };
    }

    // Extend-then-reset once, purely to observe whether the reset takes —
    // uses a throwaway uid tag so it can't collide with any real cache.
    let resettable = extend_liveness_pcr(&mut ctx, 0)
        .and_then(|_| reset_liveness_pcr(&mut ctx))
        .unwrap_or(false);

    TpmCapability {
        available: true,
        liveness_pcr_resettable: resettable,
    }
}

/// Precomputes the policy digest the seal will require at release time, by
/// *actually* extending the liveness PCR to its post-match value, capturing
/// the resulting digest via a trial session, then immediately resetting it
/// back — there is no way to compute "the digest as if a future match had
/// happened" without momentarily making it true on the real TPM (a plain
/// software hash replica would not match what the TPM itself folds in). See
/// the module docs' liveness-PCR section.
fn precompute_release_policy_digest(
    ctx: &mut Context,
    uid: u32,
) -> Result<Digest, SecretCacheError> {
    extend_liveness_pcr(ctx, uid)?;
    let slots = full_pcr_slots();
    let selection = tpm_seal::pcr_selection_for(&slots)?;
    let pcr_digest_now = tpm_seal::pcr_digest(ctx, &slots, &selection)?;
    let digest = tpm_seal::trial_pcr_policy_digest(ctx, pcr_digest_now, selection)?;

    // Whether or not the reset actually took (see reset_liveness_pcr's
    // doc), the seal step must not leave the liveness PCR sitting in its
    // post-match state — that would make the *next* boot's first release
    // attempt see a PCR that already matches without a real match having
    // happened yet.
    let _ = reset_liveness_pcr(ctx);

    Ok(digest)
}

/// Encrypts `password` with a fresh random AES-256-GCM key, seals that key
/// inside the TPM under a policy gated on a verified `context=sddm` match
/// for `uid` (see the module docs), and writes both the encrypted password
/// and the TPM-sealed key blob to disk.
///
/// Blocking (real TPM I/O) — callers on an async runtime must wrap this in
/// `tokio::task::spawn_blocking`.
pub fn seal_and_store(uid: u32, password: &str) -> Result<(), SecretCacheError> {
    let mut ctx = open_context()?;

    let key_bytes = tpm_seal::random_bytes::<32>()?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| SecretCacheError::Crypto(e.to_string()))?;
    let nonce_bytes = tpm_seal::random_bytes::<12>()?;
    let nonce: Nonce<Aes256Gcm> = Array::try_from(nonce_bytes.as_slice())
        .map_err(|_| SecretCacheError::Crypto("bad nonce length".to_string()))?;
    let ciphertext = cipher
        .encrypt(&nonce, password.as_bytes())
        .map_err(|e| SecretCacheError::Crypto(e.to_string()))?;
    let mut on_disk = Vec::with_capacity(nonce.len() + ciphertext.len());
    on_disk.extend_from_slice(&nonce);
    on_disk.extend_from_slice(&ciphertext);
    let authtok_path = encrypted_authtok_path(uid);
    if let Some(parent) = authtok_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_owner_only_file(&authtok_path, &on_disk).map_err(SecretCacheError::Io)?;

    let policy_digest = precompute_release_policy_digest(&mut ctx, uid)?;
    let sensitive_key = SensitiveData::try_from(key_bytes.to_vec())?;
    let (public, private) = tpm_seal::seal_sensitive_data(&mut ctx, sensitive_key, policy_digest)?;
    tpm_seal::write_sealed_blob(&sealed_key_path(uid), &public, &private)?;
    info!(
        "secret_cache: sealed a new session password cache for uid={}",
        uid
    );
    Ok(())
}

/// Whether `uid` already has a cached session password — a cheap existence
/// check (two `stat()`s, no TPM I/O), for reporting status to the CLI/GUI.
/// Deliberately not a plain `Path::exists()` call from those unprivileged
/// processes themselves: `/var/lib/linux-hello/secrets/` is root-only
/// (0700), so only `hello-daemon-system` can actually see into it — this is
/// exposed through the cache socket's status query instead (see
/// `pam_helper::handle_cache_request`).
pub fn has_cached_password(uid: u32) -> bool {
    sealed_key_path(uid).exists() && encrypted_authtok_path(uid).exists()
}

/// Attempts to release the cached password for `uid`, assuming a face match
/// for that uid *just* succeeded. Returns `Ok(None)` — never an error — for
/// "no cache exists" *or* for a failed unseal (policy mismatch, tampered
/// boot chain, or a corrupted/incompatible sealed blob) — every one of these
/// just means "nothing to release", handled identically by callers, rather
/// than a hard error interrupting a face-only login that would otherwise
/// have succeeded (a slightly more forgiving stance than treating a load
/// failure as fatal, taken deliberately when this was generalized alongside
/// [`crate::embedding_cipher`], which needs the same graceful degradation).
///
/// Blocking (real TPM I/O) — callers on an async runtime must wrap this in
/// `tokio::task::spawn_blocking`.
pub fn release_for_match(uid: u32) -> Result<Option<String>, SecretCacheError> {
    let sealed_path = sealed_key_path(uid);
    if !sealed_path.exists() {
        return Ok(None);
    }
    let enc_path = encrypted_authtok_path(uid);
    if !enc_path.exists() {
        return Ok(None);
    }

    let mut ctx = open_context()?;
    let (public, private) = tpm_seal::read_sealed_blob(&sealed_path)?;

    extend_liveness_pcr(&mut ctx, uid)?;
    let slots = full_pcr_slots();
    let selection = tpm_seal::pcr_selection_for(&slots)?;
    let pcr_digest_now = tpm_seal::pcr_digest(&mut ctx, &slots, &selection)?;

    let unseal_result =
        tpm_seal::unseal_with_pcr_policy(&mut ctx, public, private, pcr_digest_now, selection);

    // Restore the liveness PCR regardless of whether unseal succeeded — a
    // policy mismatch (tampered boot, or a firmware that never actually
    // reset it last time) must not leave it stuck extended forever.
    let reset_ok = reset_liveness_pcr(&mut ctx).unwrap_or(false);
    if !reset_ok {
        warn!(
            "secret_cache: liveness PCR reset did not take for uid={} — release will only work once more this boot",
            uid
        );
    }

    let aes_key_bytes = match unseal_result {
        Ok(sensitive) => Zeroizing::new(sensitive.value().to_vec()),
        Err(e) => {
            debug!(
                "secret_cache: unseal failed for uid={} (policy mismatch, tampered boot chain, or corrupted blob): {}",
                uid, e
            );
            return Ok(None);
        }
    };
    if aes_key_bytes.len() != 32 {
        return Err(SecretCacheError::Crypto(
            "unsealed key has the wrong length".to_string(),
        ));
    }
    let cipher = Aes256Gcm::new_from_slice(&aes_key_bytes)
        .map_err(|e| SecretCacheError::Crypto(e.to_string()))?;

    let on_disk = std::fs::read(&enc_path)?;
    if on_disk.len() < 12 {
        return Err(SecretCacheError::Crypto(
            "encrypted authtok truncated".to_string(),
        ));
    }
    let (nonce_bytes, ciphertext) = on_disk.split_at(12);
    let nonce: Nonce<Aes256Gcm> = Array::try_from(nonce_bytes)
        .map_err(|_| SecretCacheError::Crypto("bad nonce length".to_string()))?;
    let password = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|e| SecretCacheError::Crypto(e.to_string()))?;
    let password =
        String::from_utf8(password).map_err(|e| SecretCacheError::Crypto(e.to_string()))?;

    info!("secret_cache: released cached password for uid={}", uid);
    Ok(Some(password))
}

// Any test anywhere in this crate that points `LINUX_HELLO_SECRETS_DIR`/
// `LINUX_HELLO_TPM_TCTI` at its own tempdir must hold this lock for the
// duration — these are process-wide env vars,
// and `cargo test` runs tests from the same binary on parallel threads
// sharing one process environment. Without synchronization, one test's
// `remove_var` can fire while another is mid-`seal_and_store`, making
// `sealed_key_dir()` fall back to the real (root-only)
// `/var/lib/linux-hello/secrets` and fail with a confusing
// `PermissionDenied` — reproduced directly by running affected tests
// together without `--test-threads=1`. `pam_helper`'s cache-socket test
// shares this same lock (`crate::secret_cache::ENV_VAR_GUARD`) since it
// exercises `seal_and_store`/`release_for_match` indirectly.
//
// `tokio::sync::Mutex`, not `std::sync::Mutex`: `pam_helper`'s test is a
// `#[tokio::test]` that holds this guard across several `.await` points
// (socket read/write) — a `std` guard held there trips clippy's
// `await_holding_lock` (and would be a real footgun if this crate's async
// tests ever ran on a multi-threaded executor). Plain `#[test]`s here use
// `blocking_lock()` instead of `.lock().await` since they aren't async.
#[cfg(test)]
pub(crate) static ENV_VAR_GUARD: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(test)]
mod tests {
    use super::*;

    fn tpm_available_for_tests() -> bool {
        std::env::var("LINUX_HELLO_TPM_TCTI").is_ok()
    }

    #[test]
    fn probe_tpm_capability_reports_unavailable_without_a_configured_tcti() {
        // SAFETY: test-local env var, no other test reads it concurrently
        // within this process (each test that needs the real value sets its
        // own via LINUX_HELLO_TPM_TCTI, gated by tpm_available_for_tests()).
        if tpm_available_for_tests() {
            return; // covered by the round-trip test below instead
        }
        let cap = probe_tpm_capability();
        assert!(!cap.available);
    }

    #[test]
    fn seal_and_release_round_trip_against_a_real_or_simulated_tpm() {
        if !tpm_available_for_tests() {
            eprintln!("skipping: set LINUX_HELLO_TPM_TCTI to run against a real/simulated TPM");
            return;
        }
        let _guard = ENV_VAR_GUARD.blocking_lock();
        let secrets_dir = tempfile::tempdir().unwrap();
        std::env::set_var("LINUX_HELLO_SECRETS_DIR", secrets_dir.path());

        let uid = 999_001;
        seal_and_store(uid, "correct horse battery staple").unwrap();

        let released = release_for_match(uid).unwrap();
        assert_eq!(released.as_deref(), Some("correct horse battery staple"));

        // A second release within the same boot must also succeed — proves
        // the extend/unseal/reset cycle actually repeats, not one-shot.
        let released_again = release_for_match(uid).unwrap();
        assert_eq!(
            released_again.as_deref(),
            Some("correct horse battery staple")
        );

        std::env::remove_var("LINUX_HELLO_SECRETS_DIR");
    }

    #[test]
    fn release_for_match_returns_none_when_nothing_is_cached() {
        let _guard = ENV_VAR_GUARD.blocking_lock();
        let secrets_dir = tempfile::tempdir().unwrap();
        std::env::set_var("LINUX_HELLO_SECRETS_DIR", secrets_dir.path());
        let result = release_for_match(999_002).unwrap();
        assert!(result.is_none());
        std::env::remove_var("LINUX_HELLO_SECRETS_DIR");
    }
}
