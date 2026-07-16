#!/bin/bash
# Linux Hello PAM module installation
# Run with sudo
#
# Usage:
#   sudo ./install-pam.sh                 # configure sudo/su/polkit (idempotent; SDDM NOT touched)
#   sudo ./install-pam.sh --enable-sddm   # additionally opt in to SDDM login-screen support
#   sudo ./install-pam.sh --disable-sddm  # turn SDDM support back off (sudo/su/polkit untouched)
#   sudo ./install-pam.sh --remove        # disable everything, restore backups
#   sudo ./install-pam.sh --status        # show current status
#
# SECURITY: This script always uses "auth sufficient" — the password
# remains ALWAYS available as a fallback. You cannot be locked out.
# In case of problems: sudo ./install-pam.sh --remove
#
# NOTE: since libpam-linux-hello ships linux-hello-pam-autoconfigure (a
# systemd timer that automatically configures sudo once any user has
# enrolled a face — screenlock unlocking doesn't use PAM, see
# hello_daemon/src/screenlock.rs), running the bare form of this script is
# normally only needed for development or to explicitly opt back in after
# running --remove. SDDM is never touched by the default flow or by the
# automatic timer — it starts a root-owned, always-on, pre-authentication-
# reachable listener (hello-daemon-system.service), which is enough of a
# change to a machine's attack surface that it must always be an explicit,
# separate opt-in (--enable-sddm), whether run by hand or via the GUI's
# home-screen toggle (which invokes this script through pkexec).

set -euo pipefail

# ── Constants ────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(dirname "$(readlink -f "$0")")"
SOURCE_SO="$SCRIPT_DIR/target/release/libpam_linux_hello.so"
# Packaged location first (installed by libpam-linux-hello), falling back to
# a source checkout — same dual-lookup convention linux-hello-pam-autoconfigure
# already uses, so this script works both installed and from a dev checkout.
if [[ -f /usr/lib/linux-hello/pam-lib.sh ]]; then
    # shellcheck source=pam-lib.sh
    source /usr/lib/linux-hello/pam-lib.sh
else
    # shellcheck source=pam-lib.sh
    source "$SCRIPT_DIR/pam-lib.sh"
fi
PAM_MODULE="$(lh_module_path)"

# Packaged location first (usr/share/linux-hello/sddm/Login.qml via
# libpam-linux-hello), falling back to a source checkout — same convention
# as $SOURCE_SO above.
SDDM_QML_SOURCE="/usr/share/linux-hello/sddm/Login.qml"
if [[ ! -f "$SDDM_QML_SOURCE" ]]; then
    SDDM_QML_SOURCE="$SCRIPT_DIR/qml/sddm/Login.qml"
fi

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

    for svc in sudo sudo-i su su-l polkit-1; do
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
    lh_sddm_disable
    echo ""
    ok "Linux Hello disabled. The password takes back control."
    exit 0
fi

# ═══════════════════════════════════════════════════════════════════════════
# MODE --enable-sddm / --disable-sddm
# ═══════════════════════════════════════════════════════════════════════════
# Explicit opt-in/opt-out for SDDM login-screen support, separate from the
# default install/--remove flows — see the header comment for why. Leaves
# sudo/su/polkit entirely untouched either way.
if [[ "${1:-}" == "--enable-sddm" ]]; then
    echo "=== Enabling Linux Hello for the SDDM login screen ==="
    if [[ ! -f "$PAM_MODULE" ]]; then
        err "PAM module not installed: $PAM_MODULE"
        echo "Run 'sudo $0' first (or install libpam-linux-hello)."
        exit 1
    fi
    lh_sddm_enable "$SDDM_QML_SOURCE"
    lh_pam_autoconfig_clear_disabled
    ok "SDDM login screen: linux-hello enabled"
    exit 0
fi

if [[ "${1:-}" == "--disable-sddm" ]]; then
    echo "=== Disabling Linux Hello for the SDDM login screen ==="
    lh_sddm_disable
    ok "SDDM login screen: linux-hello disabled"
    exit 0
fi

# ═══════════════════════════════════════════════════════════════════════════
# MODE install (default) — sudo/su/polkit only. SDDM is never touched here;
# use --enable-sddm/--disable-sddm explicitly.
# ═══════════════════════════════════════════════════════════════════════════
echo "=== Linux Hello PAM Module Installation ==="
echo ""
echo "SECURITY: auth sufficient is used everywhere."
echo "The password remains ALWAYS available as a fallback."
echo "In case of problems: sudo $0 --remove"
echo ""

# ── 1. Install the .so ───────────────────────────────────────────────────────
# A dev checkout build (fresh $SOURCE_SO) always wins, so local changes get
# tested. Otherwise, if the module's already there (a packaged install placed
# it via dpkg), there's nothing to do — only a genuinely fresh machine with
# neither is an error.
if [[ -f "$SOURCE_SO" ]]; then
    mkdir -p "$(dirname "$PAM_MODULE")"
    cp "$SOURCE_SO" "$PAM_MODULE"
    chmod 644 "$PAM_MODULE"
    ok "PAM module installed: $PAM_MODULE"
elif [[ -f "$PAM_MODULE" ]]; then
    ok "PAM module already installed: $PAM_MODULE"
else
    err "Module not built: $SOURCE_SO"
    echo "Build it first (cargo build --release) or install libpam-linux-hello."
    exit 1
fi

# ── 2. sudo and sudo-i ───────────────────────────────────────────────────────
# sudo uses @include common-auth → insert BEFORE it so biometrics run first
lh_configure_service "sudo"   "sudo"   "@include common-auth" "confirm"
lh_configure_service "sudo-i" "sudo"   "@include common-auth" "confirm"

# ── 3. su and su-l ───────────────────────────────────────────────────────────
lh_configure_service "su"     "sudo"   "@include common-auth" "confirm"
lh_configure_service "su-l"   "sudo"   "@include common-auth" "confirm"

# ── 4. SDDM (login screen) — NOT done here, see header comment ──────────────
# Run 'sudo install-pam.sh --enable-sddm' explicitly (or use the GUI's
# home-screen toggle) to opt in.

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
