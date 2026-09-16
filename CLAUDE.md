# CLAUDE.md

## Shell

The user's interactive shell is **fish**, not bash. Any command meant to be pasted into their
terminal (not run via the Bash tool itself) must use fish syntax — e.g. `env VAR=value VAR2=value2 command`
instead of bash's `VAR=value command` prefix form, and `for x in a b; ...; end` instead of `for x in a b; do ...; done`.

## French UI/prompt text: formal "vous", never "tu"

All French user-facing strings in this repo (GUI i18n JSON, PAM module prompts, CLI messages) use
formal "vous" register — `pam_linux_hello`'s own `pam_t()` prompts already establish this
("Regardez vers la caméra...", "Confirmer ?"). Match it: "vous"/"votre", imperative verbs as
"-ez" (activez, appuyez, revenez), never "tu"/"ton"/"-e" tu-forms.

## In-progress feature: TPM-sealed session-password cache for KWallet auto-unlock

Branch `feature/kwallet-authtok-password-cache`, tracking
[issue #149](https://github.com/Aarklendoia/linux-hello/issues/149). Problem: a face-only SDDM
login never populates `PAM_AUTHTOK` (`pam_linux_hello.so` is `sufficient` and returns before
`pam_unix.so`/`pam_kwallet5` run), so KWallet can't auto-unlock. Fix: cache the real login
password, sealed inside the TPM, released only on an IR-liveness-confirmed `context=sddm` match,
injected via `pam_set_item`.

**Commit status** (check `git log`/`git status` for the current state — this will drift):
- Committed: the core Rust — `hello_daemon::secret_cache` (TPM sealing), the
  `PamHelperResponse`/wire-protocol changes, the cache socket, `pam_linux_hello`'s
  `pam_set_item`/`pam_sm_chauthtok` changes.
- Not yet committed as of this writing: `linux_hello_cli`'s `cache-password` subcommand
  (`cache_password.rs`), `install-pam.sh`'s new PAM service file, the GUI (`CachePassword.qml`,
  `Home.qml`'s new card, `AppController.qml` wiring, `linux_hello_config/src/main.rs`'s new
  routes), i18n strings, and `docs/PAM_MODULE.md`'s new section.

**Key design decisions, in case they need re-justifying:**
- **PCR16, not PCR23**, for the "liveness" tag extended after a face match. `tss-esapi`'s own
  upstream integration test picks PCR16 specifically as reliably resettable/extendable from
  locality 0 — confirmed empirically here too via a real seal→release→release-again round trip
  against `swtpm`.
- **SDDM-only.** Screenlock bypasses PAM entirely (`hello_daemon::screenlock` unlocks via
  `loginctl unlock-session` directly — no `pam_sm_authenticate` call to hook), and `sudo` doesn't
  need KWallet unlocked. `context=sddm` is also the only context already using the root-owned
  `hello-daemon-system` process, which is exactly the trust boundary TPM access needs.
- **No `pkexec` for `cache-password`**, unlike the SDDM toggle. Sealing a password is a request
  to an already-root, always-on daemon over a peer-uid-verified socket
  (`/run/hello-pam/cache.socket`), not a filesystem edit the calling process needs elevation to
  perform.
- The cached-password blob ends up **root-owned** (`~/.../session-authtok.enc`, written and read
  only by `hello-daemon-system`), not user-owned — intentional, not an oversight.
- **Face embedding encryption is explicitly out of scope** for this branch — a separate, harder
  problem (the per-user `hello-daemon` and root `hello-daemon-system` both need to read
  embeddings; a root-only TPM-sealed key would break the per-user daemon's own enroll/verify/
  sudo/list). Track separately once a shared-access design is decided.
- Root's own trust level isn't changed by any of this (a live root compromise was already
  equivalent to reading `/etc/shadow`) — what's new is that a compromise now yields a *reusable,
  exportable* password instead of just local control. This must ship opt-in, with an explicit
  consent warning at `cache-password` time, not enabled silently.

## Testing this feature

- `hello_daemon::secret_cache`'s real TPM round-trip test needs a TPM: either the real one
  (`/dev/tpmrm0`, root/tss-group only) or `swtpm` — start one with
  `swtpm socket --tpmstate dir=<scratch> --server type=tcp,port=2321 --tpm2 --flags not-need-init -d`,
  then run tests with `LINUX_HELLO_TPM_TCTI=swtpm:port=2321,host=127.0.0.1 cargo test -p hello_daemon`.
  Without that env var, the round-trip test just skips (prints a message) rather than failing.
- Building `hello_daemon` needs `libtss2-dev` (`sudo apt-get install -y libtss2-dev`) for
  `tss-esapi`'s headers/pkg-config files.

## GUI dev-loop gotcha: the packaged QML shadows your checkout

`linux_hello_config::find_qml_path()` checks packaged install locations
(`/usr/share/qt6/qml/Linux/Hello/main.qml` etc.) **before** falling back to the dev checkout —
if a packaged copy exists on the machine (likely, if `libpam-linux-hello`/the GUI package was ever
installed), running the built `linux_hello_config` binary normally loads the **stale packaged
QML**, not your edits. To actually exercise a local checkout, copy it into a `Linux/Hello/`
subdirectory (the module name/path `import Linux.Hello 1.0` expects) and invoke `qml6` directly
with `QML_IMPORT_PATH`/`QML2_IMPORT_PATH` pointed at the parent of that copy:

```fish
mkdir -p /tmp/lh-qml-test/Linux/Hello
cp -r linux_hello_config/qml/* /tmp/lh-qml-test/Linux/Hello/
env QML_IMPORT_PATH="/tmp/lh-qml-test:/usr/lib/x86_64-linux-gnu/qt6/qml:/usr/share/qt6/qml" \
    QML2_IMPORT_PATH="/tmp/lh-qml-test:/usr/lib/x86_64-linux-gnu/qt6/qml:/usr/share/qt6/qml" \
    QT_QUICK_CONTROLS_STYLE=org.kde.desktop \
    QML_XHR_ALLOW_FILE_READ=1 \
    qml6 /tmp/lh-qml-test/Linux/Hello/main.qml -- (id -u)
```

Any new top-level `.qml` file (a new page, e.g.) also needs a line added to
`linux_hello_config/qml/qmldir`, or `ComponentName {}` instantiation fails with
"ComponentName is not a type" — a `qmldir` file in a directory disables QML's normal
same-directory implicit-import behavior in favor of its own explicit listing.

**Do not use screenshot tools to verify GUI changes in this environment.** This machine's display
is the user's real, live desktop (multi-monitor, unrelated business apps and chat clients often
open) — screenshot tools available here (`spectacle`, ImageMagick's `import`) were only observed
capturing the *entire* desktop, not an isolated app window, which risks exposing unrelated
sensitive content (this happened once this session; the screenshots were deleted immediately).
There is also no click-automation tool (`xdotool` etc.) installed. Ask the user to run the app
themselves and describe/screenshot what they see, rather than attempting to drive or capture the
GUI directly.

## Local package testing gotcha: PackageKit silently reverts to the PPA build

This project publishes to a real Launchpad PPA (see the Launchpad-related memory entries), which
is configured as an apt source on the dev machine. A locally built `.deb` for testing (via
`dpkg-buildpackage`) inherits `debian/changelog`'s version — typically far *lower* than whatever
is already published on the PPA (e.g. local `1.0.6-1` vs. published `1.6.3~ppa1~resolute1`).
`dpkg -i`/`apt install --reinstall` will still install it, but to apt/PackageKit it now looks like
an *out-of-date* package — the next background update check (KDE Discover/PackageKit's
`packagekit role='update-packages'` job, which runs periodically and unattended) will silently
reinstall the PPA version right over it, with no prompt and no obviously-related log message
(`grep linux-hello /var/log/apt/history.log` is what actually surfaces it). This burned real
debugging time in this session: a fix looked "un-applied" on reboot when it had actually been
silently reverted minutes after installing it.

Mitigation while iterating on local builds:

```fish
sudo apt-mark hold linux-hello linux-hello-daemon linux-hello-gui linux-hello-models linux-hello-tools libpam-linux-hello
```

Run this **after** installing the local build, not before — `apt install --reinstall` (and plain
`apt install`) on a package clears its hold as a side effect, so holding first and reinstalling
second silently undoes the hold. Un-hold the same list once done testing so the PPA build (and
real users) aren't affected: `sudo apt-mark unhold ...`.

## i18n

`linux_hello_config/qml/i18n/*.json`, one file per one of 10 supported languages
(en/fr/zh/es/hi/ar/pt/ru/ja/de). When adding new keys: write real EN and FR text (this session's
established quality bar); the other 8 currently carry the **English text as a placeholder**
pending real translation — a known, flagged gap, not a finished translation.
