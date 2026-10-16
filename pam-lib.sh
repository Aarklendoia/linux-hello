#!/bin/bash
# Shared PAM configuration logic for Linux Hello.
#
# Sourced by install-pam.sh (interactive developer tool), by
# linux-hello-pam-autoconfigure (packaged, unattended activation triggered by
# the linux-hello-pam-autoconfigure.timer systemd unit) and by
# libpam-linux-hello's postinst (migration). Keeping the rules in one place
# avoids the callers drifting out of sync.
#
# Debian/Ubuntu only: face auth is wired in through a pam-auth-update profile
# (/usr/share/pam-configs/linux-hello, source: debian/pam-configs/linux-hello)
# that adds a single `pam_linux_hello.so auto` line to common-auth. The
# module derives the context from PAM_SERVICE (sudo/su -> sudo + confirm,
# polkit-1 -> polkit, sddm -> sddm only if $LH_SDDM_MARKER exists) and
# ignores every other service.
#
# Earlier versions instead inserted a marked block into each service's own
# file (/etc/pam.d/sudo, su, sddm, ...). Those files are conffiles of other
# packages (sudo-common, util-linux, sddm), so every distro update to them
# turned into a dpkg conffile prompt, and answering "install the package
# maintainer's version" silently dropped face auth (#215). Nothing here
# writes to another package's file anymore; lh_migrate_legacy_config only
# removes what those versions left behind.

# Overridable for testing (e.g. against a scratch directory instead of the
# real system paths) — unset/empty in production, where the real paths apply.
PAM_DIR="${PAM_DIR:-/etc/pam.d}"
LH_MARKER_START="# >>> linux-hello-start"
LH_MARKER_END="# <<< linux-hello-end"
LH_OPTOUT_MARKER="${LH_OPTOUT_MARKER:-/etc/linux-hello/pam-disabled}"
# SDDM opt-in marker, read by pam_linux_hello.so itself (SDDM_ENABLED_MARKER
# in pam_linux_hello/src/lib.rs, which only honours it if root-owned).
LH_SDDM_MARKER="${LH_SDDM_MARKER:-/etc/linux-hello/sddm-enabled}"
LH_PROFILE_NAME="linux-hello"
LH_PAM_CONFIGS_DIR="${LH_PAM_CONFIGS_DIR:-/usr/share/pam-configs}"
LH_PAM_AUTH_UPDATE="${LH_PAM_AUTH_UPDATE:-pam-auth-update}"
# Legacy (pre-#215) layout: services that may still carry a marked block,
# and the substack file a short-lived SDDM design used.
LH_LEGACY_SERVICES="sudo sudo-i su su-l sddm"
LH_SDDM_SUBSTACK_NAME="linux-hello-sddm-auth"
# Header of the /etc/pam.d/polkit-1 file earlier versions created from
# scratch when the distro shipped none there. It shadows polkitd's own
# /usr/lib/pam.d/polkit-1 (which @includes common-auth), so it must go.
LH_LEGACY_POLKIT_HEADER="# Linux Hello - Authentication for polkit dialogs"
LH_LOCK_FILE="${LH_LOCK_FILE:-/run/lock/linux-hello-pam.lock}"

# ── Module path ──────────────────────────────────────────────────────────────
# PAM's default module search path is multiarch-specific on Debian/Ubuntu
# (e.g. /lib/x86_64-linux-gnu/security/) — a module dropped at /lib/security/
# is silently unloadable there. Non-multiarch distros (Arch, Fedora) instead
# use a single flat /usr/lib/security/ — and `gcc -dumpmachine` doesn't even
# reliably fall back to the Debian guess there (Arch's gcc reports
# "x86_64-pc-linux-gnu", not "x86_64-linux-gnu"), so checking for the actual
# installed file at the flat path first avoids computing a nonexistent one.
lh_module_path() {
    if [[ -n "${LH_MODULE_PATH_OVERRIDE:-}" ]]; then
        echo "$LH_MODULE_PATH_OVERRIDE"
        return
    fi
    if [[ -f /usr/lib/security/pam_linux_hello.so ]]; then
        echo "/usr/lib/security/pam_linux_hello.so"
        return
    fi
    local multiarch
    multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH 2>/dev/null || gcc -dumpmachine 2>/dev/null || echo "x86_64-linux-gnu")"
    echo "/lib/${multiarch}/security/pam_linux_hello.so"
}

# ── Opt-out marker ────────────────────────────────────────────────────────────
# Set by install-pam.sh --remove, read (never written) by the unattended
# autoconfigure script, so it never fights an explicit opt-out.
lh_pam_autoconfig_disabled() {
    [[ -f "$LH_OPTOUT_MARKER" ]]
}

lh_pam_autoconfig_set_disabled() {
    mkdir -p "$(dirname "$LH_OPTOUT_MARKER")" 2>/dev/null || true
    touch "$LH_OPTOUT_MARKER"
}

lh_pam_autoconfig_clear_disabled() {
    rm -f "$LH_OPTOUT_MARKER"
}

# ── Lock file ─────────────────────────────────────────────────────────────────
# Opens fd 9 on $LH_LOCK_FILE for flock'ing. The packaged tmpfiles.d config
# (see debian/rules) pre-creates this as a root-owned regular file at every
# boot, before any user session can start — /run is tmpfs, so that closes the
# window an unprivileged local user would otherwise have to pre-plant a
# symlink at this predictable /run/lock path pointing at an arbitrary
# root-writable file. This -L check is a second layer for the case this runs
# outside the package (e.g. a dev checkout, with no tmpfiles.d entry active):
# it refuses a symlink outright rather than following it, at the cost of a
# narrow, non-attacker-acceleratable TOCTOU gap between this check and the
# exec below.
lh_acquire_lock_fd() {
    if [[ -L "$LH_LOCK_FILE" ]]; then
        echo "Refusing to use $LH_LOCK_FILE: it is a symlink, not a regular file" >&2
        exit 1
    fi
    mkdir -p "$(dirname "$LH_LOCK_FILE")" 2>/dev/null || true
    exec 9>"$LH_LOCK_FILE"
}

# ── pam-auth-update profile ──────────────────────────────────────────────────
# "Enabled" means the profile's line actually landed in common-auth — not
# pam-auth-update's exit status, which is 0 even when it declined to touch a
# locally modified common-auth (it prints "Run pam-auth-update --force to
# override" and leaves the file alone).
lh_profile_enabled() {
    grep -q "pam_linux_hello" "$PAM_DIR/common-auth" 2>/dev/null
}

# DEBIAN_FRONTEND=noninteractive: on a locally modified common-auth,
# pam-auth-update asks a high-priority debconf question, which must never
# block the unattended timer or a dpkg run. The non-interactive answer is
# "don't override", reported by the callers below as a failure with the fix
# to apply.
lh_pam_auth_update() {
    DEBIAN_FRONTEND=noninteractive "$LH_PAM_AUTH_UPDATE" "$@" < /dev/null
}

lh_profile_enable() {
    if ! command -v "$LH_PAM_AUTH_UPDATE" > /dev/null 2>&1; then
        echo "pam-auth-update not found: automatic PAM setup only supports Debian/Ubuntu (see docs/PAM_MODULE.md for manual setup)" >&2
        return 1
    fi
    if [[ ! -f "$LH_PAM_CONFIGS_DIR/$LH_PROFILE_NAME" ]]; then
        echo "PAM profile missing: $LH_PAM_CONFIGS_DIR/$LH_PROFILE_NAME (is libpam-linux-hello installed?)" >&2
        return 1
    fi
    lh_profile_enabled && return 0
    lh_pam_auth_update --enable "$LH_PROFILE_NAME" || true
    if lh_profile_enabled; then
        echo "PAM profile $LH_PROFILE_NAME: enabled (common-auth)"
        return 0
    fi
    echo "Could not enable the $LH_PROFILE_NAME PAM profile: $PAM_DIR/common-auth looks locally modified, so pam-auth-update left it alone. Run 'sudo pam-auth-update' and tick \"Linux Hello face authentication\"." >&2
    return 1
}

lh_profile_disable() {
    command -v "$LH_PAM_AUTH_UPDATE" > /dev/null 2>&1 || return 0
    lh_profile_enabled || return 0
    lh_pam_auth_update --disable "$LH_PROFILE_NAME" || true
    if lh_profile_enabled; then
        echo "Could not disable the $LH_PROFILE_NAME PAM profile ($PAM_DIR/common-auth locally modified?). Run 'sudo pam-auth-update' and untick \"Linux Hello face authentication\"." >&2
        return 1
    fi
    echo "PAM profile $LH_PROFILE_NAME: disabled"
}

# ── Legacy (pre-#215) per-service configuration ──────────────────────────────
lh_legacy_polkit_file_is_ours() {
    grep -qF "$LH_LEGACY_POLKIT_HEADER" "$PAM_DIR/polkit-1" 2>/dev/null
}

lh_has_legacy_config() {
    local svc
    for svc in $LH_LEGACY_SERVICES; do
        grep -qE "pam_linux_hello|$LH_SDDM_SUBSTACK_NAME" "$PAM_DIR/$svc" 2>/dev/null && return 0
    done
    lh_legacy_polkit_file_is_ours && return 0
    [[ -e "$PAM_DIR/$LH_SDDM_SUBSTACK_NAME" ]]
}

# Removes our marked block (and any stray pam_linux_hello / substack line)
# from one service file, in place. Leaves the rest of the file — including
# whatever the distro changed since — untouched, unlike the old
# restore-the-latest-backup approach, which could roll a conffile back to an
# outdated version.
lh_strip_legacy_lines() {
    local f="$PAM_DIR/$1"
    [[ -f "$f" ]] || return 0
    grep -qE "pam_linux_hello|$LH_SDDM_SUBSTACK_NAME|^$LH_MARKER_START\$" "$f" || return 0
    sed -i -e "/^$LH_MARKER_START\$/,/^$LH_MARKER_END\$/d" \
           -e "/pam_linux_hello/d" -e "/$LH_SDDM_SUBSTACK_NAME/d" "$f"
    echo "Service $1: legacy linux-hello lines removed"
}

lh_remove_sddm_substack() {
    rm -f "${PAM_DIR:?}/${LH_SDDM_SUBSTACK_NAME:?}"
}

lh_strip_legacy_config() {
    local svc
    for svc in $LH_LEGACY_SERVICES; do
        lh_strip_legacy_lines "$svc"
    done
    if lh_legacy_polkit_file_is_ours; then
        rm -f "${PAM_DIR:?}/polkit-1"
        echo "Service polkit-1: removed the file linux-hello created (the distro's own polkit-1 PAM config applies again)"
    fi
    lh_remove_sddm_substack
}

# Timestamped copies of distro files taken by earlier versions before
# editing them. Never restored anymore (they go stale as soon as the distro
# updates the original), so they're just clutter in /etc/pam.d.
lh_remove_legacy_backups() {
    find "${PAM_DIR:?}" -maxdepth 1 -type f -name "*.pre-linuxhello-*" -delete 2>/dev/null || true
}

# lh_migrate_legacy_config — moves a machine configured by an earlier
# version to the profile: keeps SDDM's opt-in (as the marker), enables the
# profile, and only then removes the old per-service lines, so face auth
# never has a gap. If the profile can't be enabled (locally modified
# common-auth), the old lines are left in place and this returns 1.
lh_migrate_legacy_config() {
    if lh_has_legacy_config; then
        if grep -qE "pam_linux_hello|$LH_SDDM_SUBSTACK_NAME" "$PAM_DIR/sddm" 2>/dev/null; then
            lh_sddm_marker_set
        fi
        lh_profile_enable || return 1
        lh_strip_legacy_config
    fi
    lh_remove_legacy_backups
}

# ── SDDM (login screen) enable/disable ───────────────────────────────────────
# Deliberately separate from automatic activation: SDDM starts
# hello-daemon-system.service, a root-owned, always-on,
# pre-authentication-reachable listener — explicit opt-in only, never part of
# automatic activation or the bare `install-pam.sh` default flow.
#
# No SDDM-specific PAM line anymore: /etc/pam.d/sddm already @includes
# common-auth, where the profile's line sits, and the marker is what makes
# the module act on the sddm service. On a face match, `success=end` jumps to
# the end of common-auth's primary block, so the keyring modules after it
# (pam_kwallet5's "Additional" profile entry, and sddm's own
# `-auth optional pam_kwallet5.so` / `pam_gnome_keyring.so` lines) still run
# and pick up the PAM_AUTHTOK the module set from the TPM-sealed password
# cache. That is what the previous design needed a hand-built
# `[success=ok default=3]` jump block in /etc/pam.d/sddm for.
lh_sddm_enabled() {
    [[ -f "$LH_SDDM_MARKER" ]]
}

lh_sddm_marker_set() {
    mkdir -p "$(dirname "$LH_SDDM_MARKER")" 2>/dev/null || true
    touch "$LH_SDDM_MARKER"
    chmod 0644 "$LH_SDDM_MARKER"
}

# lh_sddm_enable <sddm_qml_source>
# <sddm_qml_source> is the patched Login.qml to divert-install into the
# configured greeter theme (caller resolves the packaged-vs-checkout path).
lh_sddm_enable() {
    local sddm_qml_source="$1"

    lh_profile_enable || return 1
    lh_sddm_marker_set
    lh_strip_legacy_lines "sddm"
    lh_remove_sddm_substack
    echo "Service sddm: linux-hello enabled ($LH_SDDM_MARKER)"

    if systemctl enable --now hello-daemon-system.service 2>/dev/null; then
        echo "Service hello-daemon-system: enabled"
    else
        echo "Could not enable hello-daemon-system.service (systemd unavailable?)" >&2
    fi

    local sddm_theme sddm_login_qml
    sddm_theme="$(lh_sddm_theme_name)"
    sddm_login_qml="/usr/share/sddm/themes/$sddm_theme/Login.qml"
    if [[ ! -f "$sddm_login_qml" ]]; then
        echo "SDDM theme '$sddm_theme' has no Login.qml — status indicator not installed (login via face still works, just without visual feedback)" >&2
    elif [[ ! -f "$sddm_qml_source" ]]; then
        echo "Status indicator source missing ($sddm_qml_source) — skipped" >&2
    elif dpkg-divert --list "$sddm_login_qml" 2>/dev/null | grep -q linux-hello; then
        echo "SDDM greeter status indicator: already installed (theme: $sddm_theme)"
    else
        dpkg-divert --package libpam-linux-hello --rename --add "$sddm_login_qml" 2>/dev/null || true
        cp "$sddm_qml_source" "$sddm_login_qml"
        echo "SDDM greeter status indicator installed (theme: $sddm_theme)"
    fi
}

# lh_sddm_disable — reverses lh_sddm_enable. Leaves sudo/su/polkit (and the
# profile they share) alone.
lh_sddm_disable() {
    rm -f "${LH_SDDM_MARKER:?}"
    lh_strip_legacy_lines "sddm"
    lh_remove_sddm_substack
    echo "Service sddm: linux-hello disabled"

    if systemctl disable --now hello-daemon-system.service 2>/dev/null; then
        echo "Service hello-daemon-system: disabled"
    fi

    local sddm_theme sddm_login_qml
    sddm_theme="$(lh_sddm_theme_name)"
    sddm_login_qml="/usr/share/sddm/themes/$sddm_theme/Login.qml"
    if dpkg-divert --list "$sddm_login_qml" 2>/dev/null | grep -q linux-hello; then
        rm -f "$sddm_login_qml"
        dpkg-divert --package libpam-linux-hello --rename --remove "$sddm_login_qml" 2>/dev/null || true
        echo "SDDM greeter status indicator removed (theme: $sddm_theme)"
    fi
}

# Detects the SDDM greeter theme actually configured: last `Current=` wins,
# matching SDDM's own config-merging order (/etc/sddm.conf, then
# /etc/sddm.conf.d/*.conf in lexicographic order — the numeric prefixes
# distros use, e.g. 20-kubuntu.conf, are designed to sort correctly here).
# Falls back to "breeze", SDDM's own upstream default, if nothing is set.
lh_sddm_theme_name() {
    local theme="breeze"
    local f found
    for f in /etc/sddm.conf /etc/sddm.conf.d/*.conf; do
        [[ -f "$f" ]] || continue
        found=$(grep -E '^\s*Current\s*=' "$f" 2>/dev/null | tail -1 | cut -d'=' -f2- | xargs || true)
        [[ -n "$found" ]] && theme="$found"
    done
    echo "$theme"
}
