//! Shared, object-type-agnostic TPM2 plumbing: opening a context against a
//! chosen TCTI, the deterministic primary/parent key, PCR-policy digest
//! computation, sealing/unsealing a small blob of sensitive data under a PCR
//! policy, and the on-disk marshalling format for a sealed object's
//! `Public`/`Private` halves.
//!
//! Extracted out of [`crate::secret_cache`] (the session-password cache) so
//! [`crate::embedding_cipher`] (face-embedding encryption at rest) can reuse
//! the exact same, already-reviewed TPM code rather than duplicating it —
//! the two differ only in *which* PCRs they gate on and what they seal, not
//! in how sealing/unsealing itself works. `secret_cache`'s own public API and
//! test suite are unchanged by this extraction; its liveness-PCR extend/reset
//! dance (specific to the password cache's stronger "confirmed live match"
//! requirement) stays there, not here.

use std::convert::{TryFrom, TryInto};
use std::path::Path;

use thiserror::Error;
use tracing::warn;
use tss_esapi::{
    abstraction::pcr::PcrData,
    attributes::{ObjectAttributesBuilder, SessionAttributesBuilder},
    constants::SessionType,
    handles::KeyHandle,
    interface_types::{
        algorithm::{HashingAlgorithm, PublicAlgorithm},
        resource_handles::Hierarchy,
        session_handles::PolicySession,
    },
    structures::{
        Digest, KeyedHashScheme, MaxBuffer, PcrSelectionList, PcrSelectionListBuilder, PcrSlot,
        Private, Public, PublicBuilder, PublicKeyedHashParameters, SensitiveData,
        SymmetricDefinition,
    },
    tcti_ldr::TctiNameConf,
    traits::{Marshall, UnMarshall},
    utils, Context,
};

/// The device TCTI root should use to talk to the TPM directly.
///
/// Deliberately **not** `TctiNameConf::Device(Default::default())` — that
/// default resolves to `/dev/tpm0`, the raw device, which the kernel only
/// ever lets one process hold open at a time. `tpm2-abrmd` (needed for the
/// per-user daemon's own, unprivileged TPM access — see
/// `crate::embedding_cipher`) holds `/dev/tpm0` open for as long as it runs,
/// which is continuously once enabled. Root opening `/dev/tpm0` too would
/// race it for exclusive access instead of actually working alongside it —
/// caught for real on hardware, not in testing (`swtpm` doesn't reproduce
/// this contention). `/dev/tpmrm0`, the in-kernel resource-managed device,
/// is built for exactly this: any number of processes — the standalone
/// broker and root's own direct callers — can hold it open concurrently.
pub(crate) fn root_device_tcti() -> TctiNameConf {
    TctiNameConf::Device(
        "/dev/tpmrm0"
            .parse()
            .expect("hardcoded device path always parses"),
    )
}

#[derive(Debug, Error)]
pub(crate) enum TpmSealError {
    #[error("TPM error: {0}")]
    Tpm(#[from] tss_esapi::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("encryption error: {0}")]
    Crypto(String),
}

/// Boot-integrity PCRs — Secure Boot state (7), and the bootloader/kernel/
/// initrd measurements (8, 9) most distros' shim/GRUB chain populates. Same
/// selection `systemd-cryptenroll`'s `--tpm2-pcrs=7,8,9` default uses for
/// LUKS auto-unlock. Shared by every caller of this module — `secret_cache`
/// folds a liveness PCR in on top of this set; `embedding_cipher` uses it
/// alone.
pub(crate) const BOOT_PCR_SLOTS: [PcrSlot; 3] = [PcrSlot::Slot7, PcrSlot::Slot8, PcrSlot::Slot9];

/// Reads `N` random bytes directly from `/dev/urandom` — same technique (and
/// same rationale: `read_exact`, not `fs::read`, since the latter blocks
/// forever on a character device that never returns EOF) as
/// `security_util::generate_token`, just returning raw bytes instead of a
/// hex string.
pub(crate) fn random_bytes<const N: usize>() -> Result<[u8; N], TpmSealError> {
    use std::io::Read;
    let mut buf = [0u8; N];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut buf)?;
    Ok(buf)
}

pub(crate) fn open_context(tcti: TctiNameConf) -> Result<Context, TpmSealError> {
    let mut ctx = Context::new(tcti).map_err(|e| {
        warn!("tpm_seal: could not open TPM context: {}", e);
        TpmSealError::from(e)
    })?;
    // A real hardware TPM is already started by the firmware/kernel driver
    // before `/dev/tpmrm0` is even usable, so this fails harmlessly there
    // (TPM_RC_INITIALIZE) — ignored rather than treated as fatal. A fresh
    // swtpm state directory (as used in tests) genuinely needs this once per
    // process, or every subsequent command fails the same way.
    let _ = ctx.startup(tss_esapi::constants::StartupType::Clear);
    Ok(ctx)
}

/// Builds a `PcrSelectionList` (SHA256 bank only) for an arbitrary set of
/// PCR slots.
pub(crate) fn pcr_selection_for(slots: &[PcrSlot]) -> Result<PcrSelectionList, TpmSealError> {
    Ok(PcrSelectionListBuilder::new()
        .with_selection(HashingAlgorithm::Sha256, slots)
        .build()?)
}

/// Digest of the concatenated current PCR values for `slots`, in the given
/// order — this is exactly what `TPM2_PolicyPCR` itself folds into the
/// session's policy digest (see the doc comment on `tss-esapi`'s own
/// `common::get_pcr_policy_digest` test helper, which this mirrors), so
/// computing it the same way here lets callers both (a) precompute the
/// expected policy digest at seal time via a *trial* session, and (b)
/// authorize the real unseal at release time via a real *policy* session —
/// both calls read whatever the live PCR content is at that moment, which is
/// the whole point: they only agree when nothing has changed since sealing.
pub(crate) fn pcr_digest(
    ctx: &mut Context,
    slots: &[PcrSlot],
    selection: &PcrSelectionList,
) -> Result<Digest, TpmSealError> {
    let (_, read_selections, read_digests) = ctx.pcr_read(selection.clone())?;
    let pcr_data = PcrData::create(&read_selections, &read_digests)?;
    let bank = pcr_data
        .pcr_bank(HashingAlgorithm::Sha256)
        .ok_or_else(|| TpmSealError::Crypto("no SHA256 PCR bank returned".to_string()))?;

    let mut concatenated = Vec::new();
    for slot in slots {
        let digest = bank
            .get_digest(*slot)
            .ok_or_else(|| TpmSealError::Crypto(format!("missing PCR digest for {:?}", slot)))?;
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
/// has a handful of session slots, and callers open several sessions in
/// sequence within a single [`Context`], so leaving each one alive until the
/// whole `Context` drops exhausts them (confirmed: this failed with
/// `TPM_RC_SESSION_MEMORY` against swtpm before this function started
/// flushing eagerly).
pub(crate) fn with_pcr_auth_session<F, T>(ctx: &mut Context, f: F) -> Result<T, TpmSealError>
where
    F: FnOnce(&mut Context) -> Result<T, TpmSealError>,
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
        .ok_or_else(|| TpmSealError::Crypto("TPM returned no session handle".to_string()))?;
    let (attrs, mask) = SessionAttributesBuilder::new()
        .with_decrypt(true)
        .with_encrypt(true)
        .build();
    ctx.tr_sess_set_attributes(session, attrs, mask)?;

    let result = ctx.execute_with_session(Some(session), f);
    ctx.flush_context(tss_esapi::handles::SessionHandle::from(session).into())?;
    result
}

/// Standard RSA2048 restricted-decryption primary key template — the same
/// one `tss-esapi`'s own test suite (`common::decryption_key_pub`) uses as
/// its SRK-equivalent. Deterministic: creating a primary from this exact
/// template under the Owner hierarchy always yields the same key on a given
/// TPM, so nothing needs to be persisted for the parent itself — only the
/// sealed child object's Public/Private blobs.
pub(crate) fn primary_key(ctx: &mut Context) -> Result<KeyHandle, TpmSealError> {
    let public = utils::create_restricted_decryption_rsa_public(
        tss_esapi::abstraction::cipher::Cipher::aes_256_cfb()
            .try_into()
            .map_err(TpmSealError::from)?,
        tss_esapi::interface_types::key_bits::RsaKeyBits::Rsa2048,
        Default::default(),
    )?;
    Ok(ctx
        .execute_with_nullauth_session(|ctx| {
            ctx.create_primary(Hierarchy::Owner, public.clone(), None, None, None, None)
        })
        .map_err(TpmSealError::from)?
        .key_handle)
}

/// The `Public` template for a sealed data object, gated by `auth_policy`.
/// Deliberately `with_user_with_auth(false)`: unlike `tss-esapi`'s own test
/// template (`common::create_public_sealed_object`, which sets this `true`
/// for its own unrelated test purposes), USER-role actions on this object —
/// which is what `TPM2_Unseal` requires — must be satisfiable *only* via the
/// PCR policy, never via the object's (empty, default) plain authValue.
/// Getting this bit wrong would mean anyone could unseal the object with a
/// trivial empty-password session, bypassing the PCR gate entirely.
pub(crate) fn sealed_object_public(auth_policy: Digest) -> Result<Public, TpmSealError> {
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

/// Computes the policy digest a `TPM2_PolicyPCR` session over `selection`
/// would produce for `pcr_digest` (the already-computed digest of the PCR
/// values a future unseal must see), via a throwaway *trial* session — this
/// is what lets a seal step bake in "must match a specific future PCR state"
/// without any real policy session being open yet.
pub(crate) fn trial_pcr_policy_digest(
    ctx: &mut Context,
    pcr_digest: Digest,
    selection: PcrSelectionList,
) -> Result<Digest, TpmSealError> {
    let trial = ctx
        .start_auth_session(
            None,
            None,
            None,
            SessionType::Trial,
            SymmetricDefinition::AES_256_CFB,
            HashingAlgorithm::Sha256,
        )?
        .ok_or_else(|| TpmSealError::Crypto("TPM returned no trial session".to_string()))?;
    let trial_policy = PolicySession::try_from(trial)?;
    ctx.policy_pcr(trial_policy, pcr_digest, selection)?;
    let digest = ctx.policy_get_digest(trial_policy)?;
    ctx.flush_context(tss_esapi::handles::SessionHandle::from(trial).into())?;
    Ok(digest)
}

/// Seals `sensitive` inside the TPM under a PCR-policy-gated object with
/// `policy_digest` as its auth policy. Returns the `Public`/`Private` halves
/// to be marshalled to disk via [`write_sealed_blob`].
pub(crate) fn seal_sensitive_data(
    ctx: &mut Context,
    sensitive: SensitiveData,
    policy_digest: Digest,
) -> Result<(Public, Private), TpmSealError> {
    let public_template = sealed_object_public(policy_digest)?;
    let parent = primary_key(ctx)?;
    let created = ctx.execute_with_nullauth_session(|ctx| {
        ctx.create(parent, public_template, None, Some(sensitive), None, None)
    });
    ctx.flush_context(parent.into())?;
    let created = created?;
    Ok((created.out_public, created.out_private))
}

/// Loads a previously-sealed object and unseals it, authorized by a real
/// `TPM2_PolicyPCR` session over `selection` matching the *current* live PCR
/// values (`pcr_digest`) — succeeds only if nothing gated by `selection` has
/// changed since the object was sealed.
pub(crate) fn unseal_with_pcr_policy(
    ctx: &mut Context,
    public: Public,
    private: Private,
    pcr_digest: Digest,
    selection: PcrSelectionList,
) -> Result<SensitiveData, TpmSealError> {
    let policy = ctx
        .start_auth_session(
            None,
            None,
            None,
            SessionType::Policy,
            SymmetricDefinition::AES_256_CFB,
            HashingAlgorithm::Sha256,
        )?
        .ok_or_else(|| TpmSealError::Crypto("TPM returned no policy session".to_string()))?;
    let policy_session = PolicySession::try_from(policy)?;
    ctx.policy_pcr(policy_session, pcr_digest, selection)?;

    let parent = primary_key(ctx)?;
    let handle = ctx.execute_with_nullauth_session(|ctx| ctx.load(parent, private, public));

    let unseal_result = handle.and_then(|handle| {
        let result = ctx.execute_with_session(Some(policy), |ctx| ctx.unseal(handle.into()));
        ctx.flush_context(handle.into())?;
        result
    });

    ctx.flush_context(parent.into())?;
    ctx.flush_context(tss_esapi::handles::SessionHandle::from(policy).into())?;

    Ok(unseal_result?)
}

/// Sealed-object blob layout on disk: two length-prefixed sections
/// (marshalled `Public`, then raw `Private` bytes) — simple enough not to
/// need serde/base64 for what's fundamentally two opaque byte buffers.
pub(crate) fn write_sealed_blob(
    path: &Path,
    public: &Public,
    private: &Private,
) -> Result<(), TpmSealError> {
    let pub_bytes = public.marshall()?;
    let priv_bytes = private.value();
    let mut out = Vec::with_capacity(8 + pub_bytes.len() + priv_bytes.len());
    out.extend_from_slice(&(pub_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(&pub_bytes);
    out.extend_from_slice(&(priv_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(priv_bytes);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        // Defense in depth alongside whatever permissions the directory
        // that contains it already has: a sealed blob is useless without
        // its TPM, but the directory a *future* caller might also drop
        // other sensitive files into shouldn't rely on that alone.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    std::fs::write(path, out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub(crate) fn read_sealed_blob(path: &Path) -> Result<(Public, Private), TpmSealError> {
    let data = std::fs::read(path)?;
    if data.len() < 8 {
        return Err(TpmSealError::Crypto("sealed blob truncated".to_string()));
    }
    let pub_len = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
    let pub_end = 4 + pub_len;
    if data.len() < pub_end + 4 {
        return Err(TpmSealError::Crypto("sealed blob truncated".to_string()));
    }
    let public = Public::unmarshall(&data[4..pub_end])?;
    let priv_len_start = pub_end;
    let priv_len =
        u32::from_be_bytes(data[priv_len_start..priv_len_start + 4].try_into().unwrap()) as usize;
    let priv_start = priv_len_start + 4;
    if data.len() < priv_start + priv_len {
        return Err(TpmSealError::Crypto("sealed blob truncated".to_string()));
    }
    let private = Private::try_from(data[priv_start..priv_start + priv_len].to_vec())?;
    Ok((public, private))
}
