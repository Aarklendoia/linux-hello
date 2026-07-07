#!/bin/bash
# Linux Hello PAM module installation
# Run with sudo
#
# Usage:
#   sudo ./install-pam.sh            # install
#   sudo ./install-pam.sh --remove   # disable and restore backups
#   sudo ./install-pam.sh --status   # show current status
#
# SECURITY: This script always uses "auth sufficient" — the password
# remains ALWAYS available as a fallback. You cannot be locked out.
# In case of problems: sudo ./install-pam.sh --remove
#
# NOTE: since libpam-linux-hello ships linux-hello-pam-autoconfigure (a
# systemd timer that automatically configures sudo once any user has
# enrolled a face — screenlock unlocking doesn't use PAM, see
# hello_daemon/src/screenlock.rs), running this script manually is normally only
# needed for development (it copies the freshly built .so from this repo
# checkout) or to explicitly opt back in after running --remove.

set -euo pipefail

# ── Constants ────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"
SOURCE_SO="$SCRIPT_DIR/target/release/libpam_linux_hello.so"
# shellcheck source=pam-lib.sh
source "$SCRIPT_DIR/pam-lib.sh"
PAM_MODULE="$(lh_module_path)"

# Packaged location first (usr/share/linux-hello/sddm/Login.qml via
# libpam-linux-hello), falling back to a source checkout — same convention
# as $SOURCE_SO above.
SDDM_QML_SOURCE="/usr/share/linux-hello/sddm/Login.qml"
if [[ ! -f "$SDDM_QML_SOURCE" ]]; then
    SDDM_QML_SOURCE="$SCRIPT_DIR/qml/sddm/Login.qml"
fi

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

# ── Colors ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
ok()   { echo -e "${GREEN}✓${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC}  $*"; }
err()  { echo -e "${RED}✗${NC} $*"; }

# ── Root check ───────────────────────────────────────────────────────────────
if [[ "$EUID" -ne 0 && -z "${LH_SKIP_ROOT_CHECK:-}" ]]; then
    err "This script must be run with sudo"
    exit 1
fi

# ── Locking ──────────────────────────────────────────────────────────────────
# Guards against racing with linux-hello-pam-autoconfigure's unattended timer,
# which edits the same /etc/pam.d/* files. Interactive, so wait briefly for
# the lock rather than failing outright.
mkdir -p "$(dirname "$LH_LOCK_FILE")" 2>/dev/null || true
exec 9>"$LH_LOCK_FILE"
if ! flock -w 15 9; then
    err "Could not acquire the PAM configuration lock after 15s (another linux-hello PAM operation in progress?)"
    exit 1
fi

# ═══════════════════════════════════════════════════════════════════════════
# MODE --status
# ═══════════════════════════════════════════════════════════════════════════
if [[ "${1:-}" == "--status" ]]; then
    echo "=== linux-hello PAM module status ==="
    if [[ -f "$PAM_MODULE" ]]; then
        ok "Module installed: $PAM_MODULE"
    else
        err "Module missing: $PAM_MODULE"
    fi
    if lh_pam_autoconfig_disabled; then
        warn "Automatic activation: disabled (opt-out marker present at $LH_OPTOUT_MARKER)"
    else
        ok "Automatic activation: enabled (linux-hello-pam-autoconfigure.timer will configure sudo once a face is enrolled)"
    fi
    for svc in sudo sudo-i su su-l sddm polkit-1; do
        f="$PAM_DIR/$svc"
        if [[ -f "$f" ]]; then
            if grep -q "pam_linux_hello" "$f" 2>/dev/null; then
                ok "$svc: linux-hello enabled"
            else
                warn "$svc: linux-hello NOT configured"
            fi
        else
            echo "   $svc: file missing (normal for polkit-1)"
        fi
    done
    echo "   screenlock: handled by hello-daemon's own watcher (loginctl unlock-session), not PAM"
    sddm_theme="$(lh_sddm_theme_name)"
    if dpkg-divert --list "/usr/share/sddm/themes/$sddm_theme/Login.qml" 2>/dev/null | grep -q linux-hello; then
        ok "SDDM greeter status indicator: installed (theme: $sddm_theme)"
    else
        warn "SDDM greeter status indicator: not installed (theme: $sddm_theme)"
    fi
    exit 0
fi

# ═══════════════════════════════════════════════════════════════════════════
# MODE --remove  (disable + restore backups)
# ═══════════════════════════════════════════════════════════════════════════
if [[ "${1:-}" == "--remove" ]]; then
    echo "=== Disabling Linux Hello PAM ==="

    # Set the opt-out marker first (defense in depth even with the lock held):
    # linux-hello-pam-autoconfigure must never re-enable what was just removed.
    lh_pam_autoconfig_set_disabled
    ok "Automatic activation disabled ($LH_OPTOUT_MARKER)"

    for svc in sudo sudo-i su su-l sddm polkit-1; do
        f="$PAM_DIR/$svc"
        # Look for the most recent backup for this service
        latest_bak=$(find "$PAM_DIR" -maxdepth 1 -name "$svc.pre-linuxhello-*" -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2- || true)
        if [[ -n "$latest_bak" ]]; then
            cp "$latest_bak" "$f"
            ok "Restored: $svc ← $latest_bak"
        elif [[ -f "$f" ]]; then
            # No backup: remove the linux-hello lines via sed
            sed -i "/$LH_MARKER_START/,/$LH_MARKER_END/d" "$f"
            sed -i '/pam_linux_hello/d' "$f"
            ok "Cleaned: $svc (linux-hello lines removed)"
        fi
    done
    # polkit-1: if it was created by this script, remove it
    if grep -q "linux-hello" "$PAM_DIR/polkit-1" 2>/dev/null; then
        rm -f "$PAM_DIR/polkit-1"
        ok "Removed: polkit-1 (created by this script)"
    fi
    # SDDM system listener: stop it too — no point leaving a root, pre-auth
    # listener running once its only caller (the sddm PAM line) is gone.
    if systemctl disable --now hello-daemon-system.service 2>/dev/null; then
        ok "Service hello-daemon-system: disabled"
    fi
    # SDDM greeter status indicator: revert the theme's Login.qml if we diverted it.
    sddm_theme="$(lh_sddm_theme_name)"
    SDDM_LOGIN_QML="/usr/share/sddm/themes/$sddm_theme/Login.qml"
    if dpkg-divert --list "$SDDM_LOGIN_QML" 2>/dev/null | grep -q linux-hello; then
        rm -f "$SDDM_LOGIN_QML"
        dpkg-divert --package libpam-linux-hello --rename --remove "$SDDM_LOGIN_QML" 2>/dev/null || true
        ok "SDDM greeter status indicator removed (theme: $sddm_theme)"
    fi
    echo ""
    ok "Linux Hello disabled. The password takes back control."
    exit 0
fi

# ═══════════════════════════════════════════════════════════════════════════
# MODE install (default)
# ═══════════════════════════════════════════════════════════════════════════
echo "=== Linux Hello PAM Module Installation ==="
echo ""
echo "SECURITY: auth sufficient is used everywhere."
echo "The password remains ALWAYS available as a fallback."
echo "In case of problems: sudo $0 --remove"
echo ""

# ── 1. Install the .so ───────────────────────────────────────────────────────
if [[ ! -f "$SOURCE_SO" ]]; then
    err "Module not built: $SOURCE_SO"
    echo "Build it first: cargo build --release"
    exit 1
fi
mkdir -p "$(dirname "$PAM_MODULE")"
cp "$SOURCE_SO" "$PAM_MODULE"
chmod 644 "$PAM_MODULE"
ok "PAM module installed: $PAM_MODULE"

# ── 2. sudo and sudo-i ───────────────────────────────────────────────────────
# sudo uses @include common-auth → insert BEFORE it so biometrics run first
lh_configure_service "sudo"   "sudo"   "@include common-auth" "confirm"
lh_configure_service "sudo-i" "sudo"   "@include common-auth" "confirm"

# ── 3. su and su-l ───────────────────────────────────────────────────────────
lh_configure_service "su"     "sudo"   "@include common-auth" "confirm"
lh_configure_service "su-l"   "sudo"   "@include common-auth" "confirm"

# ── 4. SDDM (login screen) ───────────────────────────────────────────────────
# Insert before @include common-auth (after the nologin/pam_succeed_if checks).
# Backed by hello-daemon-system.service, a root-owned, always-on listener
# started at boot — a new pre-authentication-reachable attack surface, so it
# is enabled here (opt-in, alongside the PAM line itself) rather than
# unconditionally at package install time. Not part of automatic activation
# (linux-hello-pam-autoconfigure never touches this file or this service).
lh_configure_service "sddm"   "sddm"   "@include common-auth"
if systemctl enable --now hello-daemon-system.service 2>/dev/null; then
    ok "Service hello-daemon-system: enabled"
else
    warn "Could not enable hello-daemon-system.service (systemd unavailable?)"
fi

# SDDM greeter status indicator: without it, the greeter shows nothing at
# all while a face is being checked — pam_linux_hello's PAM_TEXT_INFO
# messages do reach the greeter's `sddm` QML object, but no theme actually
# has a handler for that signal, so they're silently dropped (confirmed on
# a real login this session: it succeeded via face recognition alone, with
# no visible cue, leaving the user unsure and typing a password anyway).
sddm_theme="$(lh_sddm_theme_name)"
SDDM_LOGIN_QML="/usr/share/sddm/themes/$sddm_theme/Login.qml"
if [[ ! -f "$SDDM_LOGIN_QML" ]]; then
    warn "SDDM theme '$sddm_theme' has no Login.qml — status indicator not installed (login via face still works, just without visual feedback)"
elif [[ ! -f "$SDDM_QML_SOURCE" ]]; then
    warn "Status indicator source missing ($SDDM_QML_SOURCE) — skipped"
elif dpkg-divert --list "$SDDM_LOGIN_QML" 2>/dev/null | grep -q linux-hello; then
    ok "SDDM greeter status indicator: already installed (theme: $sddm_theme)"
else
    dpkg-divert --package libpam-linux-hello --rename --add "$SDDM_LOGIN_QML" 2>/dev/null || true
    cp "$SDDM_QML_SOURCE" "$SDDM_LOGIN_QML"
    ok "SDDM greeter status indicator installed (theme: $sddm_theme)"
fi

# ── 5. polkit-1 ("Authentication required" graphical dialogs) ─────────────────
POLKIT_FILE="$PAM_DIR/polkit-1"
if [[ ! -f "$POLKIT_FILE" ]]; then
    cat > "$POLKIT_FILE" << 'EOF'
#%PAM-1.0
# Linux Hello - Authentication for polkit dialogs (pkexec, graphical elevation)
# auth sufficient = if biometrics OK → access granted; otherwise → password fallback

auth       sufficient   pam_linux_hello.so context=polkit
auth       required     pam_unix.so nullok
@include common-account
EOF
    ok "Service polkit-1: created with linux-hello + password fallback"
else
    lh_configure_service "polkit-1" "polkit" "^auth"
fi

# Screenlock unlocking doesn't use PAM: hello-daemon's own watcher polls
# org.freedesktop.ScreenSaver and unlocks via `loginctl unlock-session` on a
# face match (see hello_daemon/src/screenlock.rs) — nothing to configure here.

# ── Explicit re-enable: clear the opt-out marker on a successful run ─────────
lh_pam_autoconfig_clear_disabled

# ── Summary ──────────────────────────────────────────────────────────────────
echo ""
echo "=== Configuration summary ==="
for svc in sudo sudo-i su su-l sddm polkit-1; do
    f="$PAM_DIR/$svc"
    if [[ -f "$f" ]]; then
        if grep -q "pam_linux_hello" "$f"; then
            ok "$svc: linux-hello active"
            grep "pam_linux_hello" "$f" | sed 's/^/     /'
        else
            warn "$svc: not configured"
        fi
    fi
done
echo "   screenlock: handled by hello-daemon's own watcher (loginctl unlock-session), not PAM"

echo ""
echo "=== Quick test ==="
echo "Test with: sudo -k && sudo ls /"
echo "If locked out: sudo $0 --remove"
echo ""
ok "Installation complete."
