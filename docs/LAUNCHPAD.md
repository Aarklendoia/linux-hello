# Publishing to Launchpad (PPA)

This is a step-by-step guide to publish Linux Hello as a Launchpad PPA
(`ppa:aarklendoia-edtech/linux-hello`), so users can `apt install` it
directly instead of downloading `.deb` files from GitHub Releases.

- Launchpad account: `aarklendoia-edtech`
- Personal signing key (manual uploads): fingerprint
  `86EB1CE672402B0B104049C3D4251A0893FE3895` (`aarklendoia@proton.me`) —
  confirmed on the account, Code of Conduct signed.
- CI signing key (automated uploads, see [Automated publishing](#5-automated-publishing-ci)):
  fingerprint `78E5E953B5D26A386600270F28CCA4654276F229`
  (`aarklendoia+ci@proton.me`) — a separate, dedicated key so a leaked CI
  secret can't be used to impersonate the personal identity; confirmed on
  the same Launchpad account.
- PPA: `ppa:aarklendoia-edtech/linux-hello`
  (<https://launchpad.net/~aarklendoia-edtech/+archive/ubuntu/linux-hello>)
  — first successful build published 2026-07-10 (`linux-hello 1.1.1~ppa1~resolute1`,
  amd64, Ubuntu 26.04 LTS).

Launchpad's build farm has no general internet access, so the plain
`cargo build --release` in `debian/rules` would fail there — see
[Vendoring](#2-vendoring-required-before-every-ppa-upload) for how that's
handled.

## 1. One-time account setup (manual, on launchpad.net) — done

None of this can be automated from a script — it requires a browser and your
own identity. Kept here for reference / for setting up on another machine:

1. Create a Launchpad account at <https://launchpad.net/+login> (this doubles
   as your Ubuntu One account). ✅ `aarklendoia-edtech`
2. Generate (or reuse) a GPG key, and publish it to a keyserver:

   ```bash
   # Use an existing key, or generate one:
   gpg --full-generate-key

   # List your keys and note the fingerprint of the one you want to use:
   gpg --list-secret-keys --keyid-format long

   # Publish it (Launchpad polls keyserver.ubuntu.com):
   gpg --keyserver keyserver.ubuntu.com --send-keys <FINGERPRINT>
   ```

   ✅ Using `86EB1CE672402B0B104049C3D4251A0893FE3895`.

3. On your Launchpad profile page (`https://launchpad.net/~<username>`,
   *not* `login.launchpad.net` — that's the SSO/SSH-keys service, a
   different thing), "OpenPGP keys" section → import the same fingerprint.
   Launchpad emails a confirmation you must decrypt (`gpg --decrypt`) and
   follow the link in. ✅ confirmed.
4. Sign the [Ubuntu Code of Conduct](https://launchpad.net/codeofconduct):
   download the current text, `gpg --clearsign` it with the same key, paste
   the result back on the site. Not strictly required to upload to a
   personal PPA, but expected practice. ✅ signed.
5. Create the PPA: profile page → "Create a new PPA" → name, description,
   public visibility. ✅ `ppa:aarklendoia-edtech/linux-hello`.
6. Install the upload tooling locally:

   ```bash
   sudo apt install devscripts dput debhelper lintian gnupg
   ```

   ✅ installed.

## 2. Vendoring (required before every PPA upload)

Launchpad's build farm builds every package in an isolated environment with
**no general internet access** — builders can only reach a restricted,
whitelisted set of URLs through a proxy (mainly the Ubuntu archive itself).
`crates.io`, GitHub, and any other package registry are not reachable during
a build.

Two places in this repository assume network access is available *during*
`dpkg-buildpackage`, and both would fail on Launchpad unmodified:

1. **Cargo dependencies**: `debian/rules`' `override_dh_auto_build` runs
   `cargo build --release`, which fetches ~30 crates from crates.io on a
   clean checkout.
2. **ONNX models**: `hello_face_core/build.rs` downloads `buffalo_sc.zip`
   from a GitHub release (`deepinsight/insightface`) into
   `~/.local/share/linux-hello/models/`, and `debian/rules` copies those
   files into the `linux-hello-models` package.

Both work fine on GitHub Actions (which has internet access) and locally —
`debian/rules` still does that by default. The fix, needed only for a PPA
build, is to **vendor everything before packaging**, from a machine with
network access:

```bash
# Check the target series' packaged cargo version first:
rmadison -u ubuntu cargo | grep resolute   # or whatever series you're targeting

RUST_TOOLCHAIN=1.93.1 ./debian/scripts/prepare-offline-build.sh
```

**The vendoring toolchain must match the target series' packaged cargo
version** (installed via `rustup toolchain install <version>` if you don't
have it — the script does this for you if `RUST_TOOLCHAIN` is set). This
isn't optional: a newer local cargo vendoring the tree can silently omit
`Cargo.toml.orig` companion files that an *older* cargo needs at build time
to verify a vendored crate's checksum against `Cargo.lock`. Hit this for
real going from a local cargo 1.96.1 to resolute's 1.93.1 — the mismatched
vendor snapshot failed on Launchpad with `failed to calculate checksum of:
vendor/anyhow/Cargo.toml.orig: No such file or directory`, on every crate
whose manifest cargo 1.93.1 needed to normalize (431 of ~400 vendored
crates, in this case). Re-vendoring with `+1.93.1` fixed it — verified
locally with `cargo +1.93.1 build --workspace --offline` before
re-uploading, which is the cheapest way to catch this class of bug without
waiting on a Launchpad build round-trip.

Practically, this means **a separate vendor snapshot per target series**
if you support more than one (their packaged cargo versions differ) — not
a single one reused everywhere.

This script (never run by `debian/rules` itself, and never run on a
Launchpad builder):

- Runs `cargo vendor vendor` and writes `.cargo/config.toml` so Cargo
  resolves every dependency from the local `vendor/` directory instead of
  crates.io. The vendored tree is ~320 MB across ~400 crates — normal for
  this dependency set (`ort`, `tract-onnx`, `sqlx`, `image`, `zbus`, …).
- Pre-fetches the ONNX model pack into
  `${XDG_DATA_HOME:-$HOME/.local/share}/linux-hello/models/` — the same
  path `hello_face_core/build.rs` already checks first, so it finds the
  models "already present" and never tries to download them. No code
  change needed there.

Once it's run, `debian/rules`' `override_dh_auto_build` detects `vendor/` +
`.cargo/config.toml` in the working directory and automatically switches to
`cargo build --release --offline` — this is what actually gets uploaded to
Launchpad.

`vendor/` and `.cargo/` are git-ignored: they're a per-release snapshot
regenerated right before packaging, not part of normal `main` history (a
322 MB vendor tree in every commit would make every diff noisy for no
benefit — only the one about-to-be-uploaded source tarball needs it). Since
this repo's source format is `3.0 (native)`, `debuild -S` tars up the
working directory as-is: whatever is physically present when you run it
(vendor/, .cargo/, the pre-fetched models) gets included, regardless of
`.gitignore`. `debian/source/options` explicitly excludes `target/` from
that tarball (dpkg-source's default ignore list doesn't know about Rust
build artifacts, and `target/` can be tens of GB after a full build).

## 3. Building and uploading a release

Launchpad requires one **source-only** upload per target Ubuntu series (they
each get built separately against that series' own library versions) — you
can't upload one generic package for "Ubuntu" the way GitHub Actions builds
one generic `.deb`.

For each series you want to support (e.g. `noble` 24.04, `plucky` 25.04) —
**repeat the vendor step too**, matching that series' cargo version (see
[Vendoring](#2-vendoring-required-before-every-ppa-upload) above), not just
the changelog's target distribution:

```bash
rmadison -u ubuntu cargo | grep noble
RUST_TOOLCHAIN=<that version> ./debian/scripts/prepare-offline-build.sh

# One changelog entry per series, with a ~ppa<N>~<series><N> suffix so
# versions sort correctly and never collide with an eventual Debian/Ubuntu
# archive version. No "-1" revision: this package is "3.0 (native)"
# format, which can't carry a Debian revision number.
dch --newversion "1.1.0~ppa1~noble1" --distribution noble \
  "Automated PPA build for Ubuntu 24.04 (noble)."

# Build a signed, source-only package (needs the GPG key from step 1):
debuild -S -sa

# Upload — dput reads the changes file, uploads over SFTP, Launchpad
# builds it on its farm and publishes to the PPA once it succeeds:
dput ppa:aarklendoia-edtech/linux-hello ../linux-hello_1.1.0~ppa1~noble1_source.changes
```

Repeat the whole `prepare-offline-build.sh` (with that series' toolchain) +
`dch` + `debuild -S -sa` + `dput` cycle per series. Track build status at
<https://launchpad.net/~aarklendoia-edtech/+archive/ubuntu/linux-hello/+packages>.

## 4. Once published

Done: the README's [Quick start](../README.md#quick-start-for-everyday-users)
has the `add-apt-repository`/`apt install linux-hello` line, and the
Launchpad badge is in the badge row. `debian/changelog`'s PPA-suffixed
entries (`~ppa1~resolute1` etc.) are never committed — see
[Vendoring](#2-vendoring-required-before-every-ppa-upload) above; the
`dch` step there generates them locally, right before `debuild`, on top
of whatever release-please/[docs/RELEASE.md](RELEASE.md) already put in
`debian/changelog` for that tag.

## 5. Automated publishing (CI)

[.github/workflows/publish-ppa.yml](../.github/workflows/publish-ppa.yml)
automates the whole cycle above — triggered on every `vX.Y.Z` tag push
(same trigger as `build-debian.yml`'s GitHub Release build), or manually
via `workflow_dispatch` (useful to test against a series/toolchain
combination before relying on the automatic trigger, or to re-run after a
failure without pushing a new tag).

It signs with a **separate, CI-only GPG key** (fingerprint
`78E5E953B5D26A386600270F28CCA4654276F229`, `aarklendoia+ci@proton.me`),
not the personal one used for manual uploads — a leaked `PPA_GPG_PRIVATE_KEY`
repository secret then only grants PPA-upload rights, not the ability to
impersonate the maintainer's own identity elsewhere (git tag signing, Code
of Conduct, other services). The key:

- Has no passphrase (required — GitHub Actions can't answer a pinentry
  prompt), which is exactly why it must stay a low-privilege, single-purpose
  key and not the personal one.
- Is registered as an *additional* OpenPGP key on the same
  `aarklendoia-edtech` Launchpad account (Launchpad supports multiple keys
  per account — same `+editpgpkeys` → paste fingerprint → confirm via
  encrypted email flow as [step 1](#1-one-time-account-setup-manual-on-launchpadnet--done) above).
- Its private key (armored, `gpg --armor --export-secret-keys <fingerprint>`)
  is stored only as the `PPA_GPG_PRIVATE_KEY` GitHub Actions repository
  secret — never committed, never printed in logs.

Since GitHub Actions runners have full internet access (unlike Launchpad's
build farm), the CI job's own vendoring step (`prepare-offline-build.sh`)
is fast — no local `rustup`-juggling needed, it just installs the target
series' exact toolchain via `dtolnay/rust-toolchain` fresh each run.

To support an additional series, add a `workflow_dispatch` test run first
(pass the series codename and its `rmadison -u ubuntu cargo` version) to
confirm it builds, before considering whether to extend the tag-triggered
default beyond `resolute`.
