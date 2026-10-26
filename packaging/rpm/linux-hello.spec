#
# spec file for linux-hello
#
# One spec for openSUSE Tumbleweed and Fedora (45+), built by
# .github/workflows/publish-obs.yml and published through the Open Build
# Service (#192). Same package split as debian/ and packaging/aur/.
#
# Builds are offline (OBS requires it): Source1 holds the vendored crates,
# the cargo config pointing at them and Cargo.lock, produced by
# packaging/rpm/make-sources.sh through the same
# debian/scripts/prepare-offline-build.sh the PPA uses. Both sources have
# fixed names: the OBS package's _service fetches them from the latest
# GitHub release.
#
# Fedora 44 isn't supported: its onnxruntime 1.22 is older than the API
# level `ort` is built for (23), and aborts inside onnxruntime when built
# for 22 instead (see #192).
#
# PAM is not configured automatically, like on Arch: install-pam.sh,
# pam-lib.sh and the autoconfigure timer assume Debian's `@include
# common-auth` and dpkg-divert (#194). See docs/PAM_MODULE.md, "Manual setup
# on other distributions".
#
# The ONNX face models aren't shipped (#211): InsightFace's terms only allow
# non-commercial research use and don't grant redistribution. `sudo
# linux-hello models install` shows those terms and fetches them. The
# -models subpackage is kept, empty, so that upgrading from a version that
# shipped them keeps the local copy (%%pre models).

# The release profile has no debug info, so there'd be nothing to put in the
# debuginfo/debugsource packages (Fedora fails the build on an empty one).
%global debug_package %{nil}

%if 0%{?suse_version}
%global ort_pkg libonnxruntime1
%global abrmd_pkg tpm2.0-abrmd
%global dbus_pkg dbus-1
%else
%global ort_pkg onnxruntime
%global abrmd_pkg tpm2-abrmd
%global dbus_pkg dbus
%endif

Name:           linux-hello
# x-release-please-start-version
Version:        1.12.1
# x-release-please-end
Release:        0%{?dist}
Summary:        Face authentication for Linux (metapackage)
License:        GPL-3.0-or-later
URL:            https://github.com/Aarklendoia/linux-hello
Source0:        %{name}-source.tar.gz
Source1:        %{name}-vendor.tar.xz

BuildRequires:  cargo
BuildRequires:  clang-devel
BuildRequires:  gcc
BuildRequires:  pam-devel
BuildRequires:  rust >= 1.93
BuildRequires:  systemd-rpm-macros
BuildRequires:  pkgconfig(tss2-esys)
BuildRequires:  pkgconfig(tss2-mu)
BuildRequires:  pkgconfig(tss2-sys)
BuildRequires:  pkgconfig(tss2-tctildr)
# %%check loads the face detector through onnxruntime.
BuildRequires:  %{ort_pkg}

Requires:       %{name}-daemon = %{version}-%{release}
Requires:       %{name}-pam = %{version}-%{release}
Requires:       %{name}-tools = %{version}-%{release}

%description
Linux Hello provides facial recognition authentication (sudo, polkit,
screen unlock, and optionally the SDDM login screen), with IR liveness
detection and TPM-sealed face data. This metapackage installs the daemon,
the PAM module and the command-line tools; the GUI is in linux-hello-gui.

%package daemon
Summary:        Face authentication for Linux - daemon
# onnxruntime is loaded at runtime (dlopen), so it isn't picked up
# automatically like the libraries the binaries link against.
Requires:       %{ort_pkg}
Requires:       %{name}-models = %{version}-%{release}
Requires:       %{dbus_pkg}
Requires:       polkit
Requires:       systemd
Recommends:     %{abrmd_pkg}

%description daemon
The per-user hello-daemon: camera capture, face detection and matching,
the D-Bus API the CLI and GUI use, and screen unlock. Enabled for every
user through a systemd user preset, and started in open sessions on install.

%package models
Summary:        ONNX models for Linux Hello face authentication
BuildArch:      noarch

%description models
Linux Hello's face models (InsightFace's SCRFD-500M detector and ArcFace
MobileNetV3 embedding, from the buffalo_sc pack) are licensed for
non-commercial research only and not included. Install them after reading
those terms with: sudo linux-hello models install

%package pam
Summary:        PAM module for Linux Hello
Requires:       %{ort_pkg}
Requires:       %{name}-daemon = %{version}-%{release}
Requires:       %{name}-tools = %{version}-%{release}
Requires:       pam

%description pam
pam_linux_hello.so and hello-daemon-system, the root listener used for the
SDDM login screen. PAM isn't configured automatically: see
/usr/share/doc/linux-hello-daemon/PAM_MODULE.md, "Manual setup on other
distributions".

%package tools
Summary:        Command-line tools for Linux Hello
Requires:       %{name}-daemon = %{version}-%{release}
# `linux-hello models install`.
Requires:       curl
Requires:       unzip

%description tools
The linux-hello CLI: enroll, verify, list and delete faces.

%package gui
Summary:        Configuration GUI for Linux Hello
Requires:       %{name}-daemon = %{version}-%{release}
Requires:       hicolor-icon-theme
Requires:       polkit
# The UI runs in Qt's own QML runtime (qml6, or qml-qt6 on Fedora, where
# only the -devel package ships it), with the org.kde.desktop style.
%if 0%{?suse_version}
Requires:       kf6-kirigami-imports
Requires:       qt6-declarative-tools
%else
Requires:       kf6-kirigami
Requires:       qt6-qtdeclarative-devel
%endif
Requires:       kf6-qqc2-desktop-style

%description gui
Qt/Kirigami settings application: enroll and manage faces, camera preview,
password caching for KWallet auto-unlock.

%prep
%autosetup -p1 -a1 -n %{name}-%{version}

%build
# Fedora's %%build exports RUSTFLAGS with -Ccodegen-units=1 -Cdebuginfo=2 for
# every crate: that overrides the workspace profile (and the PR builds'
# no-LTO settings), slows the whole build down, and puts full debug info into
# the fat-LTO links, which already need 5+ GB each. Let the workspace profile
# decide, as on Debian, Arch and openSUSE.
unset RUSTFLAGS
export CARGO_HOME="$PWD/.cargo-home"
export CARGO_NET_OFFLINE=true
# The models aren't shipped (see the header): build.rs mustn't fetch them.
export LINUX_HELLO_NO_MODEL_DOWNLOAD=1
# One final artifact per cargo call: the dependencies compile in parallel,
# but the fat-LTO final links (5+ GB each) run one at a time — see the
# PKGBUILD's build() for the numbers.
for target in '--bin hello-daemon' '--bin hello-daemon-system' \
              '--bin linux-hello' '--bin linux_hello_config' \
              '--package pam_linux_hello --lib'; do
  cargo build --release --offline $target
done

%install
install -Dm755 target/release/hello-daemon %{buildroot}%{_bindir}/hello-daemon
install -Dm755 target/release/hello-daemon-system %{buildroot}%{_bindir}/hello-daemon-system
install -Dm755 target/release/linux-hello %{buildroot}%{_bindir}/linux-hello
install -Dm755 target/release/linux_hello_config %{buildroot}%{_bindir}/linux_hello_config
install -Dm755 target/release/libpam_linux_hello.so %{buildroot}%{_libdir}/security/pam_linux_hello.so

# The units point ORT_DYLIB_PATH at Debian's multiarch path: use this
# distro's soname symlink instead.
install -Dm644 hello-daemon.service %{buildroot}%{_userunitdir}/hello-daemon.service
install -Dm644 hello-daemon-system.service %{buildroot}%{_unitdir}/hello-daemon-system.service
sed -i 's|^Environment=ORT_DYLIB_PATH=.*|Environment=ORT_DYLIB_PATH=%{_libdir}/libonnxruntime.so.1|' \
  %{buildroot}%{_userunitdir}/hello-daemon.service \
  %{buildroot}%{_unitdir}/hello-daemon-system.service

install -Dm644 debian/polkit/com.linuxhello.manage-faces.policy \
  %{buildroot}%{_datadir}/polkit-1/actions/com.linuxhello.manage-faces.policy
# Lets an ordinary user's hello-daemon reach tpm2-abrmd (#152).
install -Dm644 debian/dbus/linux-hello-tabrmd.conf \
  %{buildroot}%{_datadir}/dbus-1/system.d/linux-hello-tabrmd.conf
# Same entries as debian/rules (see it for the rationale).
install -d %{buildroot}%{_tmpfilesdir}
printf 'd /run/hello-pam 1777 root root -\nf /run/lock/linux-hello-camera.lock 0666 root root -\nf /run/lock/linux-hello-pam.lock 0600 root root -\nd /var/lib/linux-hello/secrets 0700 root root -\n' \
  > %{buildroot}%{_tmpfilesdir}/linux-hello.conf
# Enables the per-user daemon for every account, not just the installer's,
# like the Debian package does since #203: %%systemd_user_post applies it.
install -d %{buildroot}%{_userpresetdir}
echo 'enable hello-daemon.service' > %{buildroot}%{_userpresetdir}/80-linux-hello.preset
install -Dm644 linux-hello.config.toml.example \
  %{buildroot}%{_sysconfdir}/linux-hello/config.toml.example

install -Dm644 linux-hello.desktop %{buildroot}%{_datadir}/applications/linux-hello.desktop
install -Dm644 linux-hello.metainfo.xml %{buildroot}%{_datadir}/metainfo/linux-hello.metainfo.xml
install -Dm644 linux_hello_config/qml/icons/app-icon.svg \
  %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/linux-hello.svg
# Distro-neutral location linux_hello_config's find_qml_path() checks.
install -d %{buildroot}%{_datadir}/linux-hello/qml-modules/Linux/Hello
cp -r linux_hello_config/qml/* %{buildroot}%{_datadir}/linux-hello/qml-modules/Linux/Hello/

%check
unset RUSTFLAGS
export CARGO_HOME="$PWD/.cargo-home"
export CARGO_NET_OFFLINE=true
# Without the models, the tests that would load them skip themselves.
export LINUX_HELLO_NO_MODEL_DOWNLOAD=1
export ORT_DYLIB_PATH=%{_libdir}/libonnxruntime.so.1
# The workspace's no-LTO `ci` profile, in its own target/ci/: tests don't
# need the shipped binaries' fat LTO.
cargo test --offline --workspace --profile ci

%if 0%{?suse_version}
%pre daemon
# openSUSE's systemd-update-helper only applies the preset (in %%post) to
# units marked here first; Fedora's applies it without this, and doesn't
# define the macro.
%systemd_user_pre hello-daemon.service
%endif

%post daemon
%tmpfiles_create linux-hello.conf
%systemd_user_post hello-daemon.service
# The preset only takes effect at each user's next login: also (re)start the
# daemon in every user session already open, as debian/linux-hello-daemon.postinst
# does. A failure for one user must not fail the install.
for runtime_dir in /run/user/*; do
  [ -S "$runtime_dir/bus" ] || continue
  uid=$(basename "$runtime_dir")
  [ "$uid" -ge 1000 ] 2>/dev/null || continue
  user=$(id -nu "$uid" 2>/dev/null) || continue
  runuser -u "$user" -- env XDG_RUNTIME_DIR="$runtime_dir" \
    sh -c 'systemctl --user daemon-reload && systemctl --user restart hello-daemon.service' \
    >/dev/null 2>&1 || :
done

%preun daemon
%systemd_user_preun hello-daemon.service
if [ $1 -eq 0 ]; then
  for runtime_dir in /run/user/*; do
    [ -S "$runtime_dir/bus" ] || continue
    user=$(id -nu "$(basename "$runtime_dir")" 2>/dev/null) || continue
    runuser -u "$user" -- env XDG_RUNTIME_DIR="$runtime_dir" \
      systemctl --user stop hello-daemon.service >/dev/null 2>&1 || :
  done
fi

# Upgrade from a version that shipped the models: runs before the old
# package's files are removed, and keeps the copy this machine already has
# where `linux-hello models install` would put it.
%pre models
old=%{_datadir}/linux-hello/models
new=%{_sharedstatedir}/linux-hello/models
if [ -f "$old/det_500m.onnx" ] && [ -f "$old/w600k_mbf.onnx" ] &&
   ! { [ -f "$new/det_500m.onnx" ] && [ -f "$new/w600k_mbf.onnx" ]; }; then
  install -d -m755 "$new"
  install -m644 "$old/det_500m.onnx" "$old/w600k_mbf.onnx" "$new/"
fi

%post models
if ! { [ -f %{_sharedstatedir}/linux-hello/models/det_500m.onnx ] &&
       [ -f %{_sharedstatedir}/linux-hello/models/w600k_mbf.onnx ]; }; then
  echo "linux-hello: the face models are not included (InsightFace's terms only"
  echo "allow non-commercial research use). Face authentication stays off until"
  echo "you read those terms and install them: sudo linux-hello models install"
fi

%post pam
%systemd_post hello-daemon-system.service

%preun pam
%systemd_preun hello-daemon-system.service

%postun pam
%systemd_postun_with_restart hello-daemon-system.service

%files
%license LICENSE
%doc README.md

%files daemon
%license LICENSE
%doc README.md docs/QUICKSTART.md docs/INTEGRATION_GUIDE.md docs/PAM_MODULE.md
%{_bindir}/hello-daemon
%{_userunitdir}/hello-daemon.service
%{_datadir}/polkit-1/actions/com.linuxhello.manage-faces.policy
%{_datadir}/dbus-1/system.d/linux-hello-tabrmd.conf
%{_tmpfilesdir}/linux-hello.conf
%{_userpresetdir}/80-linux-hello.preset
%dir %{_sysconfdir}/linux-hello
%config(noreplace) %{_sysconfdir}/linux-hello/config.toml.example

%files models
%license LICENSE

%files pam
%license LICENSE
%{_libdir}/security/pam_linux_hello.so
%{_bindir}/hello-daemon-system
%{_unitdir}/hello-daemon-system.service

%files tools
%license LICENSE
%{_bindir}/linux-hello

%files gui
%license LICENSE
%{_bindir}/linux_hello_config
%{_datadir}/applications/linux-hello.desktop
%{_datadir}/metainfo/linux-hello.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/linux-hello.svg
%dir %{_datadir}/linux-hello
%{_datadir}/linux-hello/qml-modules/

%changelog
