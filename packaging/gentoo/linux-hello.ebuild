# Copyright 2026 Gentoo Authors
# Distributed under the terms of the GNU General Public License v2

# Template: packaging/gentoo/make-overlay.sh fills CRATES (and the crate part
# of LICENSE) from a freshly resolved Cargo.lock, since Cargo.lock isn't
# committed upstream.

EAPI=8

CRATES="
"

RUST_MIN_VER="1.93.0"
inherit cargo desktop optfeature pam systemd tmpfiles xdg

DESCRIPTION="Face authentication for Linux: sudo, polkit, screen unlock and SDDM login"
HOMEPAGE="https://github.com/Aarklendoia/linux-hello"
# The ONNX models: same pinned InsightFace pack the PKGBUILD, the RPM spec
# and hello_face_core/build.rs use (make-overlay.sh checks its sha256 before
# the Manifest records it).
SRC_URI="
	https://github.com/Aarklendoia/linux-hello/archive/refs/tags/v${PV}.tar.gz
		-> ${P}.tar.gz
	https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_sc.zip
		-> ${PN}-insightface-buffalo_sc-0.7.zip
	${CARGO_CRATE_URIS}
"

# The models are InsightFace's, for non-commercial research only (#211).
LICENSE="GPL-3+ InsightFace-research"
# Dependent crate licenses
LICENSE+=""
SLOT="0"
KEYWORDS="~amd64"
IUSE="+gui +pam"
RESTRICT="bindist mirror"

COMMON_DEPEND="
	app-crypt/tpm2-tss:=
	pam? ( sys-libs/pam )
"
DEPEND="${COMMON_DEPEND}"
# onnxruntime is loaded at runtime (dlopen) and only packaged in ::guru.
# The GUI runs in Qt's own QML runtime with the org.kde.desktop style. The
# daemon is a systemd user unit, but systemd isn't a hard dependency (Gentoo
# convention, and it isn't installable on OpenRC profiles): see pkg_postinst.
RDEPEND="${COMMON_DEPEND}
	sci-libs/onnxruntime
	sys-apps/dbus
	sys-auth/polkit
	gui? (
		dev-qt/qtdeclarative:6
		kde-frameworks/kirigami:6
		kde-frameworks/qqc2-desktop-style:6
	)
"
# libclang for tss-esapi-sys's bindgen.
BDEPEND="
	app-arch/unzip
	llvm-core/clang
	virtual/pkgconfig
"

src_prepare() {
	default
	# hello_face_core's build.rs looks for the models under
	# $XDG_DATA_HOME/linux-hello/models and downloads them otherwise.
	mkdir -p "${WORKDIR}/xdg/linux-hello/models" || die
	cp "${WORKDIR}"/{det_500m,w600k_mbf}.onnx "${WORKDIR}/xdg/linux-hello/models/" || die
}

src_compile() {
	local -x XDG_DATA_HOME="${WORKDIR}/xdg"
	# One final artifact per cargo call: the dependencies compile in parallel,
	# but the fat-LTO final links (5+ GB each) run one at a time — see the
	# PKGBUILD's build() for the numbers.
	cargo_src_compile --bin hello-daemon
	cargo_src_compile --bin linux-hello
	use pam && cargo_src_compile --bin hello-daemon-system
	use pam && cargo_src_compile --package pam_linux_hello --lib
	use gui && cargo_src_compile --bin linux_hello_config
}

src_test() {
	# The workspace's no-LTO `ci` profile, in its own target/ci/: tests don't
	# need the shipped binaries' fat LTO. Without the models (build.rs told
	# not to fetch them), the tests that would load onnxruntime skip
	# themselves, so this doesn't need it installed.
	local -x LINUX_HELLO_NO_MODEL_DOWNLOAD=1
	cargo_env "${CARGO}" test --offline --workspace --profile ci || die "cargo test failed"
}

src_install() {
	local bin="$(cargo_target_dir)"

	dobin "${bin}"/hello-daemon "${bin}"/linux-hello
	systemd_douserunit hello-daemon.service
	# The unit points ORT_DYLIB_PATH at Debian's multiarch path.
	sed -i "s|^Environment=ORT_DYLIB_PATH=.*|Environment=ORT_DYLIB_PATH=${EPREFIX}/usr/$(get_libdir)/libonnxruntime.so.1|" \
		"${D}$(systemd_get_userunitdir)/hello-daemon.service" || die

	insinto /usr/share/polkit-1/actions
	doins debian/polkit/com.linuxhello.manage-faces.policy
	# Lets an ordinary user's hello-daemon reach tpm2-abrmd (#152).
	insinto /usr/share/dbus-1/system.d
	doins debian/dbus/linux-hello-tabrmd.conf
	# Same entries as debian/rules (see it for the rationale).
	cat > "${T}/linux-hello.conf" <<-EOF || die
		d /run/hello-pam 1777 root root -
		f /run/lock/linux-hello-camera.lock 0666 root root -
		f /run/lock/linux-hello-pam.lock 0600 root root -
		d /var/lib/linux-hello/secrets 0700 root root -
	EOF
	dotmpfiles "${T}/linux-hello.conf"

	insinto /usr/share/linux-hello/models
	doins "${WORKDIR}"/{det_500m,w600k_mbf}.onnx
	insinto /etc/linux-hello
	doins linux-hello.config.toml.example

	dodoc README.md docs/{QUICKSTART,INTEGRATION_GUIDE,PAM_MODULE}.md
	# pkg_postinst points to it: keep it readable as is.
	docompress -x "/usr/share/doc/${PF}/PAM_MODULE.md"

	if use pam; then
		newpammod "${bin}"/libpam_linux_hello.so pam_linux_hello.so
		dobin "${bin}"/hello-daemon-system
		systemd_dounit hello-daemon-system.service
		sed -i "s|^Environment=ORT_DYLIB_PATH=.*|Environment=ORT_DYLIB_PATH=${EPREFIX}/usr/$(get_libdir)/libonnxruntime.so.1|" \
			"${D}$(systemd_get_systemunitdir)/hello-daemon-system.service" || die
	fi

	if use gui; then
		dobin "${bin}"/linux_hello_config
		domenu linux-hello.desktop
		insinto /usr/share/metainfo
		doins linux-hello.metainfo.xml
		newicon -s scalable linux_hello_config/qml/icons/app-icon.svg linux-hello.svg
		# Distro-neutral location linux_hello_config's find_qml_path() checks.
		insinto /usr/share/linux-hello/qml-modules/Linux/Hello
		doins -r linux_hello_config/qml/.
	fi
}

pkg_postinst() {
	tmpfiles_process linux-hello.conf
	xdg_pkg_postinst

	elog "Start the per-user daemon (as each user who'll use face login):"
	elog "  systemctl --user enable --now hello-daemon.service"
	elog "linux-hello is built around systemd (user units, logind): without it,"
	elog "start hello-daemon yourself in each session."
	if use pam; then
		elog
		elog "PAM is NOT configured automatically. After enrolling a face, follow"
		elog "\"Manual setup on other distributions\" in"
		elog "  ${EROOT}/usr/share/doc/${PF}/PAM_MODULE.md"
	fi
	elog
	elog "The bundled face models are InsightFace's, licensed for"
	elog "non-commercial research only."
	optfeature "TPM-sealed face embeddings for the per-user daemon" app-crypt/tpm2-abrmd
}
