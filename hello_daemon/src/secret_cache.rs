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

use std::convert::{TryFrom, TryInto};
use std::path::{Path, PathBuf};

use aes_gcm::aead::{array::Array, Aead, KeyInit, Nonce};
use aes_gcm::Aes256Gcm;
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use tracing::{debug, info, warn};
use tss_esapi::{
    abstraction::pcr::PcrData,
    attributes::{ObjectAttributesBuilder, SessionAttributesBuilder},
    constants::SessionType,
    handles::{KeyHandle, PcrHandle},
    interface_types::{
        algorithm::{HashingAlgorithm, PublicAlgorithm},
        resource_handles::Hierarchy,
        session_handles::PolicySession,
    },
    structures::{
        Digest, DigestValues, KeyedHashScheme, MaxBuffer, Private, Public, PublicBuilder,
        PublicKeyedHashParameters, PcrSelectionList, PcrSelectionListBuilder, PcrSlot,
        SensitiveData, SymmetricDefinition,
    },
    tcti_ldr::TctiNameConf,
    traits::{Marshall, UnMarshall},
    utils,
    Context,
};
use zeroize::Zeroizing;

use crate::security_util::write_owner_only_file;

/// Reads `N` random bytes directly from `/dev/urandom` — same technique (and
/// same rationale: `read_exact`, not `fs::read`, since the latter blocks
/// forever on a character device that never returns EOF) as
/// `security_util::generate_token`, just returning raw bytes instead of a
/// hex string.
fn random_bytes<const N: usize>() -> Result<[u8; N], SecretCacheError> {
    use std::io::Read;
    let mut buf = [0u8; N];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut buf)?;
    Ok(buf)
}

/// The dedicated liveness PCR — see the module docs above for why PCR16 was
/// chosen over the originally-proposed PCR23.
const LIVENESS_PCR_HANDLE: PcrHandle = PcrHandle::Pcr16;
const LIVENESS_PCR_SLOT: PcrSlot = PcrSlot::Slot16;

/// Boot-integrity PCRs folded into the same policy — Secure Boot state
/// (7), and the bootloader/kernel/initrd measurements (8, 9) most distros'
/// shim/GRUB chain populates. Same selection `systemd-cryptenroll`'s
/// `--tpm2-pcrs=7,8,9` default uses for LUKS auto-unlock.
const BOOT_PCR_SLOTS: [PcrSlot; 3] = [PcrSlot::Slot7, PcrSlot::Slot8, PcrSlot::Slot9];

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
        std::env::var("LINUX_HELLO_SECRETS_DIR").unwrap_or_else(|_| "/var/lib/linux-hello/secrets".to_string()),
    )
}

fn sealed_key_path(uid: u32) -> PathBuf {
    sealed_key_dir().join(format!("{}.tpm-sealed", uid))
}

/// Encrypted password blob, placed under the user's own home directory tree
/// (same layout convention as `hello_daemon::storage`'s per-user paths) but
/// actually written and read only by `hello-daemon-system` running as root
/// — this file ends up root-owned, mode 0600, not user-owned. That's
/// intentional, not an oversight: only the root process ever needs to touch
/// it (both `seal_and_store` and `release_for_match` run inside
/// `hello-daemon-system`), so there's no reason for the owning user's own
/// account to have direct read access to a file that's useless without the
/// TPM-sealed key sitting in `/var/lib/linux-hello/secrets/` anyway.
fn encrypted_authtok_path(uid: u32) -> Result<PathBuf, SecretCacheError> {
    let home = match std::env::var("LINUX_HELLO_TEST_HOME_OVERRIDE") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => crate::pam_helper::resolve_home_dir(uid)
            .ok_or_else(|| SecretCacheError::Io(std::io::Error::other("no home directory for uid")))?,
    };
    Ok(home
        .join(".local/share/linux-hello/users")
        .join(uid.to_string())
        .join("session-authtok.enc"))
}

fn tcti_conf() -> Result<TctiNameConf, SecretCacheError> {
    if let Ok(spec) = std::env::var("LINUX_HELLO_TPM_TCTI") {
        return spec.parse().map_err(|_| SecretCacheError::NoTpm);
    }
    Ok(TctiNameConf::Device(Default::default()))
}

fn open_context() -> Result<Context, SecretCacheError> {
    let mut ctx = Context::new(tcti_conf()?).map_err(|e| {
        warn!("secret_cache: could not open TPM context: {}", e);
        SecretCacheError::from(e)
    })?;
    // A real hardware TPM is already started by the firmware/kernel driver
    // before `/dev/tpmrm0` is even usable, so this fails harmlessly there
    // (TPM_RC_INITIALIZE) — ignored rather than treated as fatal. A fresh
    // swtpm state directory (as used in tests) genuinely needs this once per
    // process, or every subsequent command fails the same way.
    let _ = ctx.startup(tss_esapi::constants::StartupType::Clear);
    Ok(ctx)
}

/// The two-role PCR selection (boot-integrity + liveness), SHA256 bank only.
fn pcr_selection() -> Result<PcrSelectionList, SecretCacheError> {
    let mut slots = BOOT_PCR_SLOTS.to_vec();
    slots.push(LIVENESS_PCR_SLOT);
    Ok(PcrSelectionListBuilder::new()
        .with_selection(HashingAlgorithm::Sha256, &slots)
        .build()?)
}

/// Standard RSA2048 restricted-decryption primary key template — the same
/// one `tss-esapi`'s own test suite (`common::decryption_key_pub`) uses as
/// its SRK-equivalent. Deterministic: creating a primary from this exact
/// template under the Owner hierarchy always yields the same key on a given
/// TPM, so nothing needs to be persisted for the parent itself — only the
/// sealed child object's Public/Private blobs (see [`SealedBlob`]).
fn primary_key(ctx: &mut Context) -> Result<KeyHandle, SecretCacheError> {
    let public = utils::create_restricted_decryption_rsa_public(
        tss_esapi::abstraction::cipher::Cipher::aes_256_cfb()
            .try_into()
            .map_err(SecretCacheError::from)?,
        tss_esapi::interface_types::key_bits::RsaKeyBits::Rsa2048,
        Default::default(),
    )?;
    Ok(ctx
        .execute_with_nullauth_session(|ctx| {
            ctx.create_primary(Hierarchy::Owner, public.clone(), None, None, None, None)
        })
        .map_err(SecretCacheError::from)?
        .key_handle)
}

/// Digest of the concatenated current PCR values for `selection`, in
/// ascending slot order — this is exactly what `TPM2_PolicyPCR` itself folds
/// into the session's policy digest (see the doc comment on
/// `tss-esapi`'s own `common::get_pcr_policy_digest` test helper, which this
/// mirrors), so computing it the same way here lets us both (a) precompute
/// the expected policy digest at seal time via a *trial* session, and (b)
/// authorize the real unseal at release time via a real *policy* session —
/// both calls read whatever the live PCR content is at that moment, which is
/// the whole point: they only agree when nothing has changed since sealing.
fn current_pcr_digest(
    ctx: &mut Context,
    selection: &PcrSelectionList,
) -> Result<Digest, SecretCacheError> {
    let (_, read_selections, read_digests) = ctx.pcr_read(selection.clone())?;
    let pcr_data = PcrData::create(&read_selections, &read_digests)?;
    let bank = pcr_data
        .pcr_bank(HashingAlgorithm::Sha256)
        .ok_or_else(|| SecretCacheError::Crypto("no SHA256 PCR bank returned".to_string()))?;

    let mut concatenated = Vec::new();
    for slot in BOOT_PCR_SLOTS.iter().chain(std::iter::once(&LIVENESS_PCR_SLOT)) {
        let digest = bank
            .get_digest(*slot)
            .ok_or_else(|| SecretCacheError::Crypto(format!("missing PCR digest for {:?}", slot)))?;
        concatenated.extend_from_slice(digest.value());
    }

    let (hashed, _ticket) = ctx.hash(
        MaxBuffer::try_from(concatenated)?,
        HashingAlgorithm::Sha256,
        Hierarchy::Owner,
    )?;
    Ok(hashed)
}

/// Runs `f` under a one-shot HMAC session used purely to authorize
/// `pcr_extend`/`pcr_reset` (both PCR's default authValue is empty, but the
/// TPM still requires *some* authorization session for these — see
/// `tss-esapi`'s own `test_pcr_extend_reset_commands`, which uses the same
/// pattern), then flushes the session immediately — a real TPM/swtpm only
/// has a handful of session slots, and this module's callers open several
/// sessions in sequence within a single [`Context`], so leaving each one
/// alive until the whole `Context` drops exhausts them (confirmed: this
/// failed with `TPM_RC_SESSION_MEMORY` against swtpm before this function
/// started flushing eagerly).
fn with_pcr_auth_session<F, T>(ctx: &mut Context, f: F) -> Result<T, SecretCacheError>
where
    F: FnOnce(&mut Context) -> Result<T, SecretCacheError>,
{
    let session = ctx
        .start_auth_session(
            None,
            None,
            None,
            SessionType::Hmac,
            SymmetricDefinition::AES_256_CFB,
            HashingAlgorithm::Sha256,
        )?
        .ok_or_else(|| SecretCacheError::Crypto("TPM returned no session handle".to_string()))?;
    let (attrs, mask) = SessionAttributesBuilder::new()
        .with_decrypt(true)
        .with_encrypt(true)
        .build();
    ctx.tr_sess_set_attributes(session, attrs, mask)?;

    let result = ctx.execute_with_session(Some(session), f);
    ctx.flush_context(tss_esapi::handles::SessionHandle::from(session).into())?;
    result
}

fn extend_liveness_pcr(ctx: &mut Context, uid: u32) -> Result<(), SecretCacheError> {
    let tag = format!("linux-hello:auth-ok:{}", uid);
    let event_digest = Digest::try_from(Sha256::digest(tag.as_bytes()).to_vec())?;
    let mut values = DigestValues::new();
    values.set(HashingAlgorithm::Sha256, event_digest);

    with_pcr_auth_session(ctx, |ctx| Ok(ctx.pcr_extend(LIVENESS_PCR_HANDLE, values)?))
}

/// Resets the liveness PCR back to baseline. Returns `Ok(false)` (not an
/// error) if the reset call itself succeeded but the PCR didn't actually go
/// back to zero — some TPM firmwares silently ignore a locality-0 reset on
/// PCRs other than the one they've hardcoded — the caller degrades to
/// "release worked once this boot" rather than treating it as fatal.
fn reset_liveness_pcr(ctx: &mut Context) -> Result<bool, SecretCacheError> {
    with_pcr_auth_session(ctx, |ctx| Ok(ctx.pcr_reset(LIVENESS_PCR_HANDLE)?))?;

    let selection = PcrSelectionListBuilder::new()
        .with_selection(HashingAlgorithm::Sha256, &[LIVENESS_PCR_SLOT])
        .build()?;
    let (_, read_selections, read_digests) = ctx.pcr_read(selection)?;
    let pcr_data = PcrData::create(&read_selections, &read_digests)?;
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
        Err(_) => return TpmCapability { available: false, liveness_pcr_resettable: false },
    };
    if pcr_selection().and_then(|sel| current_pcr_digest(&mut ctx, &sel)).is_err() {
        return TpmCapability { available: false, liveness_pcr_resettable: false };
    }

    // Extend-then-reset once, purely to observe whether the reset takes —
    // uses a throwaway uid tag so it can't collide with any real cache.
    let resettable = extend_liveness_pcr(&mut ctx, 0)
        .and_then(|_| reset_liveness_pcr(&mut ctx))
        .unwrap_or(false);

    TpmCapability { available: true, liveness_pcr_resettable: resettable }
}

/// Sealed-object blob layout on disk: two length-prefixed sections
/// (marshalled `Public`, then raw `Private` bytes) — simple enough not to
/// need serde/base64 for what's fundamentally two opaque byte buffers.
fn write_sealed_blob(path: &Path, public: &Public, private: &Private) -> Result<(), SecretCacheError> {
    let pub_bytes = public.marshall()?;
    let priv_bytes = private.value();
    let mut out = Vec::with_capacity(8 + pub_bytes.len() + priv_bytes.len());
    out.extend_from_slice(&(pub_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(&pub_bytes);
    out.extend_from_slice(&(priv_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(priv_bytes);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn read_sealed_blob(path: &Path) -> Result<(Public, Private), SecretCacheError> {
    let data = std::fs::read(path)?;
    if data.len() < 8 {
        return Err(SecretCacheError::Crypto("sealed blob truncated".to_string()));
    }
    let pub_len = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
    let pub_end = 4 + pub_len;
    if data.len() < pub_end + 4 {
        return Err(SecretCacheError::Crypto("sealed blob truncated".to_string()));
    }
    let public = Public::unmarshall(&data[4..pub_end])?;
    let priv_len_start = pub_end;
    let priv_len =
        u32::from_be_bytes(data[priv_len_start..priv_len_start + 4].try_into().unwrap()) as usize;
    let priv_start = priv_len_start + 4;
    if data.len() < priv_start + priv_len {
        return Err(SecretCacheError::Crypto("sealed blob truncated".to_string()));
    }
    let private = Private::try_from(data[priv_start..priv_start + priv_len].to_vec())?;
    Ok((public, private))
}

/// The `Public` template for the sealed AES-key object, gated by
/// `auth_policy`. Deliberately `with_user_with_auth(false)`: unlike
/// `tss-esapi`'s own test template (`common::create_public_sealed_object`,
/// which sets this `true` for its own unrelated test purposes), USER-role
/// actions on this object — which is what `TPM2_Unseal` requires — must be
/// satisfiable *only* via the PCR policy, never via the object's (empty,
/// default) plain authValue. Getting this bit wrong would mean anyone could
/// unseal the key with a trivial empty-password session, bypassing the PCR
/// gate entirely.
fn sealed_object_public(auth_policy: Digest) -> Result<Public, SecretCacheError> {
    let object_attributes = ObjectAttributesBuilder::new()
        .with_fixed_tpm(true)
        .with_fixed_parent(true)
        .with_no_da(true)
        .with_user_with_auth(false)
        .with_admin_with_policy(true)
        .build()?;

    Ok(PublicBuilder::new()
        .with_public_algorithm(PublicAlgorithm::KeyedHash)
        .with_name_hashing_algorithm(HashingAlgorithm::Sha256)
        .with_object_attributes(object_attributes)
        .with_auth_policy(auth_policy)
        .with_keyed_hash_parameters(PublicKeyedHashParameters::new(KeyedHashScheme::Null))
        .with_keyed_hash_unique_identifier(Default::default())
        .build()?)
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
    let selection = pcr_selection()?;
    let pcr_digest = current_pcr_digest(ctx, &selection)?;

    let trial = ctx
        .start_auth_session(
            None,
            None,
            None,
            SessionType::Trial,
            SymmetricDefinition::AES_256_CFB,
            HashingAlgorithm::Sha256,
        )?
        .ok_or_else(|| SecretCacheError::Crypto("TPM returned no trial session".to_string()))?;
    let trial_policy = PolicySession::try_from(trial)?;
    ctx.policy_pcr(trial_policy, pcr_digest, selection)?;
    let digest = ctx.policy_get_digest(trial_policy)?;
    ctx.flush_context(tss_esapi::handles::SessionHandle::from(trial).into())?;

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

    let key_bytes = random_bytes::<32>()?;
    let cipher =
        Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| SecretCacheError::Crypto(e.to_string()))?;
    let nonce_bytes = random_bytes::<12>()?;
    let nonce: Nonce<Aes256Gcm> = Array::try_from(nonce_bytes.as_slice())
        .map_err(|_| SecretCacheError::Crypto("bad nonce length".to_string()))?;
    let ciphertext = cipher
        .encrypt(&nonce, password.as_bytes())
        .map_err(|e| SecretCacheError::Crypto(e.to_string()))?;
    let mut on_disk = Vec::with_capacity(nonce.len() + ciphertext.len());
    on_disk.extend_from_slice(&nonce);
    on_disk.extend_from_slice(&ciphertext);
    let authtok_path = encrypted_authtok_path(uid)?;
    if let Some(parent) = authtok_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_owner_only_file(&authtok_path, &on_disk).map_err(SecretCacheError::Io)?;

    let policy_digest = precompute_release_policy_digest(&mut ctx, uid)?;
    let public_template = sealed_object_public(policy_digest)?;
    let parent = primary_key(&mut ctx)?;
    let sensitive_key = SensitiveData::try_from(key_bytes.to_vec())?;
    let created = ctx.execute_with_nullauth_session(|ctx| {
        ctx.create(parent, public_template, None, Some(sensitive_key), None, None)
    })?;
    ctx.flush_context(parent.into())?;

    write_sealed_blob(&sealed_key_path(uid), &created.out_public, &created.out_private)?;
    info!("secret_cache: sealed a new session password cache for uid={}", uid);
    Ok(())
}

/// Attempts to release the cached password for `uid`, assuming a face match
/// for that uid *just* succeeded. Returns `Ok(None)` — never an error — for
/// "no cache exists", so callers treat that identically to nothing-to-do.
///
/// Blocking (real TPM I/O) — callers on an async runtime must wrap this in
/// `tokio::task::spawn_blocking`.
pub fn release_for_match(uid: u32) -> Result<Option<String>, SecretCacheError> {
    let sealed_path = sealed_key_path(uid);
    if !sealed_path.exists() {
        return Ok(None);
    }
    let enc_path = encrypted_authtok_path(uid)?;
    if !enc_path.exists() {
        return Ok(None);
    }

    let mut ctx = open_context()?;
    let (public, private) = read_sealed_blob(&sealed_path)?;

    extend_liveness_pcr(&mut ctx, uid)?;
    let selection = pcr_selection()?;
    let pcr_digest = current_pcr_digest(&mut ctx, &selection)?;

    let policy = ctx
        .start_auth_session(
            None,
            None,
            None,
            SessionType::Policy,
            SymmetricDefinition::AES_256_CFB,
            HashingAlgorithm::Sha256,
        )?
        .ok_or_else(|| SecretCacheError::Crypto("TPM returned no policy session".to_string()))?;
    let policy_session = PolicySession::try_from(policy)?;
    ctx.policy_pcr(policy_session, pcr_digest, selection)?;

    let parent = primary_key(&mut ctx)?;
    let handle = ctx.execute_with_nullauth_session(|ctx| ctx.load(parent, private, public))?;

    let unseal_result = ctx.execute_with_session(Some(policy), |ctx| ctx.unseal(handle.into()));

    ctx.flush_context(handle.into())?;
    ctx.flush_context(parent.into())?;
    ctx.flush_context(tss_esapi::handles::SessionHandle::from(policy).into())?;

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
                "secret_cache: unseal failed for uid={} (policy mismatch or tampered boot chain): {}",
                uid, e
            );
            return Ok(None);
        }
    };
    if aes_key_bytes.len() != 32 {
        return Err(SecretCacheError::Crypto("unsealed key has the wrong length".to_string()));
    }
    let cipher = Aes256Gcm::new_from_slice(&aes_key_bytes)
        .map_err(|e| SecretCacheError::Crypto(e.to_string()))?;

    let on_disk = std::fs::read(&enc_path)?;
    if on_disk.len() < 12 {
        return Err(SecretCacheError::Crypto("encrypted authtok truncated".to_string()));
    }
    let (nonce_bytes, ciphertext) = on_disk.split_at(12);
    let nonce: Nonce<Aes256Gcm> = Array::try_from(nonce_bytes)
        .map_err(|_| SecretCacheError::Crypto("bad nonce length".to_string()))?;
    let password = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|e| SecretCacheError::Crypto(e.to_string()))?;
    let password = String::from_utf8(password)
        .map_err(|e| SecretCacheError::Crypto(e.to_string()))?;

    info!("secret_cache: released cached password for uid={}", uid);
    Ok(Some(password))
}

// Any test anywhere in this crate that points `LINUX_HELLO_SECRETS_DIR`/
// `LINUX_HELLO_TEST_HOME_OVERRIDE`/`LINUX_HELLO_TPM_TCTI` at its own tempdir
// must hold this lock for the duration — these are process-wide env vars,
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
        let home_dir = tempfile::tempdir().unwrap();
        let secrets_dir = tempfile::tempdir().unwrap();
        std::env::set_var("LINUX_HELLO_TEST_HOME_OVERRIDE", home_dir.path());
        std::env::set_var("LINUX_HELLO_SECRETS_DIR", secrets_dir.path());

        let uid = 999_001;
        seal_and_store(uid, "correct horse battery staple").unwrap();

        let released = release_for_match(uid).unwrap();
        assert_eq!(released.as_deref(), Some("correct horse battery staple"));

        // A second release within the same boot must also succeed — proves
        // the extend/unseal/reset cycle actually repeats, not one-shot.
        let released_again = release_for_match(uid).unwrap();
        assert_eq!(released_again.as_deref(), Some("correct horse battery staple"));

        std::env::remove_var("LINUX_HELLO_TEST_HOME_OVERRIDE");
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
