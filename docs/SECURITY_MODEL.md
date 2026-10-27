# Security model

This document lists what Linux Hello protects, against whom, how, and what it
doesn't protect against. It's meant for reviewers, auditors and contributors
changing anything on an authentication path. Every mitigation points to the
code that implements it. The reasoning behind each choice is in those places
and in [PAM_MODULE.md](PAM_MODULE.md); this page is the map.

To report a vulnerability, see [SECURITY.md](../SECURITY.md).

## Assets

| Asset | Where | Why it matters |
| --- | --- | --- |
| The authentication decision | `pam_linux_hello.so` in the calling process (sudo, su, polkit agent, SDDM) | A wrong "success" grants a session or root |
| Face embeddings | `~/.local/share/linux-hello/` (per-user daemon's copy), root's own copy for SDDM | Lets someone enroll a lookalike or measure a face; biometric data can't be changed like a password |
| Cached login password (opt-in) | TPM-sealed blob, written and read only by `hello-daemon-system` | A reusable, exportable credential, not only a session |
| The camera | `/dev/video*` | Privacy: turning it on reveals who is in front of the machine |

## Components and trust boundaries

```
 calling process (sudo / su / polkit agent / SDDM)
   └─ pam_linux_hello.so ──unix socket──▶ per-user hello-daemon   (runs as the user)
                         └─unix socket──▶ hello-daemon-system      (root; SDDM, cross-session polkit)
 linux-hello CLI / settings app ──session D-Bus──▶ per-user hello-daemon
 per-user hello-daemon ──▶ camera, models, tpm2-abrmd (embedding key)
 hello-daemon-system   ──▶ camera, models, /dev/tpmrm0 (embedding key, password cache)
```

- **PAM module ↔ daemons**: `/run/hello-pam/<uid>.socket` (per-user, mode
  0666) and `/run/hello-pam/system.socket` (root, mode 0600). Both ends check
  the peer's uid (`SO_PEERCRED`). The module only trusts the target user's uid
  on the per-user socket, and root on the system socket
  (`call_pam_helper_sync`, `pam_linux_hello/src/lib.rs`), because
  `/run/hello-pam` is world-writable (1777) and could be squatted. The
  per-user daemon accepts root, the target user, `polkitd` and `nobody` (the
  sudo-rs sandbox) as callers (`handle_pam_request`,
  `hello_daemon/src/pam_helper.rs`).
- **Clients ↔ per-user daemon**: the session bus, reachable only by the same
  uid. `register_face`/`delete_face` also need a fresh polkit check
  (`com.linuxhello.manage-faces`, `hello_daemon/src/authz.rs`), so a process
  that briefly runs as the user can't silently plant a face that then unlocks
  `sudo`. See [DBUS_API.md](DBUS_API.md).
- **Local HTTP endpoints** (`127.0.0.1`: MJPEG preview, lock-screen control):
  reachable by every local user, so each request needs a token stored in
  `$XDG_RUNTIME_DIR` (mode 0700 directory, owner-only file)
  (`hello_daemon/src/preview.rs`, `hello_daemon/src/screenlock.rs`).
- **SDDM**: `hello-daemon-system` is root, always on and reachable before any
  login, so it's opt-in only: a root-owned marker `/etc/linux-hello/sddm-enabled`
  that the module ignores unless it's owned by root (`sddm_enabled`).
- **PAM wiring**: one `pam-auth-update` profile line in `common-auth`,
  `[success=end default=ignore]`. Any failure falls through to the
  password, so face authentication can't lock anyone out, and the module
  never edits another package's conffiles (#215).

## Attackers and threats

### Someone physically at the machine

| Threat | Mitigation | Remaining risk |
| --- | --- | --- |
| Photo, video or screen replay of the user's face | Liveness gate on every attempt (`matcher::match_with_liveness`): IR texture/gradient score with an IR camera, RGB heuristic without | **RGB-only liveness is weak**: a phone-screen replay scored as more textured than a live face in testing (see [PAM_MODULE.md, Anti-spoofing](PAM_MODULE.md#anti-spoofing--liveness)). The GUI and `CameraInfo` warn when there's no IR camera |
| 3D mask, IR-capable spoof | IR score | Not evaluated |
| Lookalike / twin | ArcFace threshold per context (`matcher.rs`), two consecutive matching frames (`REQUIRED_CONSECUTIVE_MATCHES`) | Inherent to face recognition at this model size |
| The user is just visible to the camera (`sudo` run by someone else at their keyboard) | `sudo`/`su` require an explicit `[y/N]` after a match. The lock screen only tries after keyboard/mouse activity, never on a glance | polkit dialogs and SDDM don't ask for confirmation |
| Releasing the cached password with a spoof | Released only for `context=sddm` **and** a match that used IR liveness (`compute_verify_response`); never on the RGB fallback | — |
| Learning which accounts use face login at the greeter | System-socket failures are padded to `timeout_ms` (`respond_with_floor`) | Small residual timing differences, documented in PAM_MODULE.md |

### A local unprivileged user (another account)

| Threat | Mitigation |
| --- | --- |
| Squatting `/run/hello-pam/<uid>.socket` or `system.socket` to answer "success" | Module-side peer-uid check; refused and falls back to the password (covered by `tests/pam-e2e`) |
| Asking another user's daemon to verify or enroll | Session bus is per-user; daemon-side peer-uid check on the PAM socket |
| Turning on SDDM face login | Marker must be owned by root |
| Reading another user's preview frames | Loopback endpoints need a token in the owner's runtime dir |
| Racing on lock files in `/run/lock` | Created ahead of time by tmpfiles.d |

### Malware running as the user

| Threat | Mitigation | Remaining risk |
| --- | --- | --- |
| Enrolling its operator's face to keep `sudo` access | Interactive polkit check on register/delete | A user who types their password in that prompt approves the change |
| Calling `Verify` | Only returns a score to the caller; grants nothing | Turns the camera on (LED) |
| Reading embeddings | Encrypted with a TPM-sealed key when a TPM and `tpm2-abrmd` are available (`embedding_cipher.rs`, #152); `EmbeddingEncryptionInfo` reports it | **Without a TPM they're stored unencrypted**, readable by any process of the user. With one, the key's policy is the boot state only (no authorization value), so a process of the user can unseal it through `tpm2-abrmd` too: the encryption is against offline theft, not against the user's own processes |

### A process running as `nobody` or `polkitd`

Accepted by the per-user PAM socket because sudo-rs and polkit agents run
their authentication there. Such a process can make the user's daemon run a
verification (turning the camera on) and learn whether the user is in front of
it. That's a privacy gap, not an authentication one: the answer doesn't
authenticate anyone by itself.

### Stolen disk or backup

| Asset | Protection |
| --- | --- |
| Embeddings | Sealed key bound to PCR7 (Secure Boot state); useless on other hardware or with a different Secure Boot configuration |
| Cached password | Sealed in the TPM; also needs the liveness tag in PCR16, which only a match extends |

With Secure Boot disabled, PCR7 protects much less. See
[PAM_MODULE.md, Known limitations](PAM_MODULE.md#known-limitations-accepted-not-solved).

### Root

Out of scope: root can already read `/etc/shadow` and load PAM modules. The
password cache does change one thing, stated in the README's security notes:
a live root compromise can recover the **literal** password (by extending
PCR16 itself) rather than only reset it. That's why the cache is opt-in,
with a consent warning.

## Supply chain and models

- The face models aren't shipped (#211): `linux-hello models install` checks
  a pinned SHA-256 before installing them in `/var/lib/linux-hello/models`.
- Without the models the daemon refuses to capture (`CameraError::ModelsMissing`).
  It never uses the stub detector/extractor, which would accept almost anyone.
- No network access during authentication. Inference is local (ONNX Runtime /
  tract).

## Known gaps

1. RGB-only liveness is a coarse triage, not anti-spoofing on par with IR.
2. Without a TPM, embeddings are unencrypted at rest.
3. `nobody`/`polkitd` processes can trigger a verification on the per-user
   socket (privacy).
4. polkit and SDDM grant on a match without confirmation.
5. No external audit yet. The fuzzing targets (`hello_face_core/fuzz`) only
   cover the liveness scoring functions; the socket protocol is covered by unit
   tests and `tests/pam-e2e`, not fuzzed.

Any change to the components above should update this page.
