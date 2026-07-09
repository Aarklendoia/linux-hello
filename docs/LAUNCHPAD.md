# Publishing to Launchpad (PPA)

This is a step-by-step guide to publish Linux Hello as a Launchpad PPA
(`ppa:aarklendoia-edtech/linux-hello`), so users can `apt install` it
directly instead of downloading `.deb` files from GitHub Releases.

- Launchpad account: `aarklendoia-edtech`
- Signing key: fingerprint `86EB1CE672402B0B104049C3D4251A0893FE3895`
  (`aarklendoia@proton.me`) — confirmed on the account, Code of Conduct
  signed.
- PPA: `ppa:aarklendoia-edtech/linux-hello`
  (<https://launchpad.net/~aarklendoia-edtech/+archive/ubuntu/linux-hello>)
  — created, currently empty (see the blocker below for why nothing's been
  uploaded yet).

**Read the [blocker](#blocker-launchpad-builders-have-no-general-internet-access)
section before investing time in the account/PPA setup below** — as this
repository is built today, a Launchpad build will fail partway through, and
fixing that is a separate, non-trivial piece of work from what's described in
the rest of this guide.

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

## Blocker: Launchpad builders have no general internet access

Launchpad's build farm builds every package in an isolated environment with
**no general internet access** — builders can only reach a restricted,
whitelisted set of URLs through a proxy (mainly the Ubuntu archive itself).
`crates.io`, GitHub, and any other package registry are not reachable during
a build.

Two places in this repository currently assume network access is available
*during* `dpkg-buildpackage`, and both will fail on Launchpad as-is:

1. **Cargo dependencies**: `debian/rules`' `override_dh_auto_build` runs
   `cargo build --release`, which fetches ~30 crates from crates.io on a
   clean checkout. Works fine on GitHub Actions (which does have internet
   access) and locally — not on Launchpad.
2. **ONNX models**: `hello_face_core/build.rs` downloads
   `buffalo_sc.zip` from a GitHub release
   (`deepinsight/insightface`) into `~/.local/share/linux-hello/models/`,
   and `debian/rules` (lines 87–92) copies those files into the
   `linux-hello-models` package. No network, no models, no package.

This is normal for any Rust/Go/Node project on Launchpad, not specific to
this codebase — the standard fix is to **vendor everything into the source
tarball** so the build only ever touches local files:

- `cargo vendor` — downloads all crate sources into a `vendor/` directory
  and prints the `[source]` replacement block for `.cargo/config.toml`;
  both get committed/shipped as part of the upstream source tarball. Given
  this workspace's dependency tree (`ort`, `tract-onnx`, `sqlx`, `image`,
  `zbus`, …), expect the vendored tree to be large (tens to a few hundred
  MB) — Launchpad accepts large source uploads, but it makes every source
  diff noisy, so this is usually done as a separate "vendor snapshot" step
  in the release process rather than committed to the normal `main`
  history.
- The ONNX model files need to ship as a second, pre-fetched "orig"
  tarball (a `get-orig-source` step, or a manually prepared
  `linux-hello_<version>.orig-models.tar.gz` component) instead of being
  downloaded by `build.rs` — with `LINUX_HELLO_NO_MODEL_DOWNLOAD=1` set so
  the build script doesn't even try, and `debian/rules` updated to copy
  from that pre-fetched location instead of `~/.local/share/...`.

None of that is done here — it's a distinct piece of packaging work (a day
or so, not a quick follow-up) from the README/badges/licensing changes in
this pass. Say the word if you want it tackled next.

## 2. Building and uploading a release (once the blocker above is resolved)

Launchpad requires one **source-only** upload per target Ubuntu series (they
each get built separately against that series' own library versions) — you
can't upload one generic package for "Ubuntu" the way GitHub Actions builds
one generic `.deb`.

For each series you want to support (e.g. `noble` 24.04, `plucky` 25.04):

```bash
# From a clean checkout, one changelog entry per series, with a
# ~ppa<N>~<series><N> suffix so versions sort correctly and never collide
# with an eventual Debian/Ubuntu archive version:
dch --newversion "1.1.0-1~ppa1~noble1" --distribution noble \
  "Automated PPA build for Ubuntu 24.04 (noble)."

# Build a signed, source-only package (needs the GPG key from step 1):
debuild -S -sa

# Upload — dput reads the changes file, uploads over SFTP, Launchpad
# builds it on its farm and publishes to the PPA once it succeeds:
dput ppa:aarklendoia-edtech/linux-hello ../linux-hello_1.1.0-1~ppa1~noble1_source.changes
```

Repeat the `dch` + `debuild -S -sa` + `dput` cycle per series. Track build
status at
<https://launchpad.net/~aarklendoia-edtech/+archive/ubuntu/linux-hello/+packages>.

## 3. Once published

- Add an install line to the README:

  ```bash
  sudo add-apt-repository ppa:aarklendoia-edtech/linux-hello
  sudo apt update
  sudo apt install linux-hello
  ```

- Add a Launchpad badge next to the others in [README.md](../README.md), e.g.
  `[![Launchpad PPA](https://img.shields.io/badge/PPA-linux--hello-orange)](https://launchpad.net/~aarklendoia-edtech/+archive/ubuntu/linux-hello)`.
- Keep `debian/changelog`'s PPA-suffixed entries (`~ppa1~noble1` etc.) out of
  the entries release-please manages on `main` — the automated release flow
  in [docs/RELEASE.md](RELEASE.md) generates a plain `X.Y.Z-1` entry per
  GitHub tag; PPA uploads should branch off that with the series suffix
  added on top, not replace it.
