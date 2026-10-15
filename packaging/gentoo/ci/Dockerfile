# CI image for .github/workflows/build-gentoo.yml (#193): a Gentoo stage3
# with the Portage tree, ::guru, the binhost set up, and sys-auth/linux-hello's
# dependencies and the packaging tooling already installed, so a run only
# builds the package itself. Same approach as kio-protondrive's image.
#
# sci-libs/onnxruntime is only in ::guru, which the binhost doesn't cover:
# it's built from source here (the slow part of this image, along with its
# ::guru dependencies), once per image. The workflow rebuilds and pushes the
# image monthly or when this file changes (its tag is the month plus this
# file's hash), not weekly like kio-protondrive's: onnxruntime makes each
# rebuild long.
FROM gentoo/stage3:systemd

# GitHub's runners have no IPv6, but Portage's wget sometimes only tries
# distfiles.gentoo.org's IPv6 addresses and the whole emerge fails.
RUN echo 'inet4_only = on' >> /etc/wgetrc

# The official binhost provides KF6/Qt6/Rust/LLVM prebuilt for this profile.
# pkgcheck only reads /etc/portage/repos.conf, not Portage's built-in
# default for ::gentoo. Tests (src_test) are enabled for this package only:
# a global FEATURES=test turns USE=test on for every dependency, which no
# longer matches the binhost's packages.
RUN emerge-webrsync --quiet \
 && eselect profile set default/linux/amd64/23.0/desktop/plasma/systemd \
 && getuto \
 && echo 'FEATURES="${FEATURES} getbinpkg binpkg-request-signature"' >> /etc/portage/make.conf \
 && mkdir -p /etc/portage/package.accept_keywords /etc/portage/package.use /etc/portage/repos.conf /etc/portage/env \
 && cp /usr/share/portage/config/repos.conf /etc/portage/repos.conf/gentoo.conf \
 && printf '%s\n' 'sys-auth/linux-hello ~amd64' 'sci-libs/onnxruntime ~amd64' 'sci-ml/onnx ~amd64' 'dev-cpp/safeint ~amd64' \
      > /etc/portage/package.accept_keywords/linux-hello \
 && echo 'sci-ml/onnx disableStaticReg' > /etc/portage/package.use/onnxruntime \
 && echo 'FEATURES="test"' > /etc/portage/env/test.conf \
 && echo 'sys-auth/linux-hello test.conf' > /etc/portage/package.env

# ::guru, for sci-libs/onnxruntime.
RUN emerge --quiet-build --getbinpkg --usepkg --noreplace app-eselect/eselect-repository dev-vcs/git \
 && eselect repository enable guru \
 && emaint sync -r guru

# The ebuild's dependencies (all USE flags on, see
# packaging/gentoo/linux-hello.ebuild) and the tooling the workflow uses. A
# dependency the ebuild gains later is still installed by the workflow's
# emerge, just not ahead of time. Portage's sandboxes need privileges
# `docker build` doesn't have; the workflow's container keeps them
# (--privileged). MAKEOPTS: Portage builds on one core otherwise, which
# onnxruntime makes very slow.
RUN MAKEOPTS="-j$(nproc)" FEATURES="-ipc-sandbox -network-sandbox -pid-sandbox" \
    emerge --quiet-build --getbinpkg --usepkg --noreplace \
      sci-libs/onnxruntime \
      app-crypt/tpm2-tss \
      sys-libs/pam \
      sys-auth/polkit \
      llvm-core/clang \
      dev-lang/rust-bin \
      app-arch/unzip \
      dev-qt/qtdeclarative:6 \
      kde-frameworks/kirigami:6 \
      kde-frameworks/qqc2-desktop-style:6 \
      app-portage/pycargoebuild \
      app-portage/portage-utils \
      dev-util/pkgcheck \
 && rm -rf /var/cache/binhost/* /var/cache/binpkgs/* /var/cache/distfiles/* /var/tmp/portage/*
