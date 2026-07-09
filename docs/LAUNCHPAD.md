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
./debian/scripts/prepare-offline-build.sh
```

This script (never run by `debian/rules` itself, and never run on a
Launchpad builder):

- Runs `cargo vendor vendor` and writes `.cargo/config.toml` so Cargo
  resolves every dependency from the local `vendor/` directory instead of
  crates.io (confirmed working with `cargo build --workspace --offline`
  against a throwaway `CARGO_HOME` — see the commit that added this script
  for how it was verified). The vendored tree is ~320 MB across ~400
  crates — normal for this dependency set (`ort`, `tract-onnx`, `sqlx`,
  `image`, `zbus`, …).
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

For each series you want to support (e.g. `noble` 24.04, `plucky` 25.04):

```bash
# Vendor once per release (not per series — the vendored tree doesn't
# change between series, only the changelog's target distribution does):
./debian/scripts/prepare-offline-build.sh

# One changelog entry per series, with a ~ppa<N>~<series><N> suffix so
# versions sort correctly and never collide with an eventual Debian/Ubuntu
# archive version:
dch --newversion "1.1.0-1~ppa1~noble1" --distribution noble \
  "Automated PPA build for Ubuntu 24.04 (noble)."

# Build a signed, source-only package (needs the GPG key from step 1):
debuild -S -sa

# Upload — dput reads the changes file, uploads over SFTP, Launchpad
# builds it on its farm and publishes to the PPA once it succeeds:
dput ppa:aarklendoia-edtech/linux-hello ../linux-hello_1.1.0-1~ppa1~noble1_source.changes
```

Repeat the `dch` + `debuild -S -sa` + `dput` cycle per series (no need to
re-run `prepare-offline-build.sh` each time, unless `Cargo.lock` changed
since). Track build status at
<https://launchpad.net/~aarklendoia-edtech/+archive/ubuntu/linux-hello/+packages>.

## 4. Once published

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
