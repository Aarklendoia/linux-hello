#!/bin/bash
# Shared PAM configuration logic for Linux Hello.
#
# Sourced by install-pam.sh (interactive developer tool) and by
# linux-hello-pam-autoconfigure (packaged, unattended activation triggered by
# the linux-hello-pam-autoconfigure.timer systemd unit). Keeping the actual
# /etc/pam.d editing rules in one place avoids the two callers drifting out
# of sync.

# Overridable for testing (e.g. against a scratch directory instead of the
# real system paths) — unset/empty in production, where the real paths apply.
PAM_DIR="${PAM_DIR:-/etc/pam.d}"
LH_MARKER_START="# >>> linux-hello-start"
LH_MARKER_END="# <<< linux-hello-end"
LH_LINE_TEMPLATE="auth       sufficient   pam_linux_hello.so context=%CONTEXT%"
LH_OPTOUT_MARKER="${LH_OPTOUT_MARKER:-/etc/linux-hello/pam-disabled}"
LH_LOCK_FILE="${LH_LOCK_FILE:-/run/lock/linux-hello-pam.lock}"
LH_TIMESTAMP="$(date +%s)"

# ── Module path ──────────────────────────────────────────────────────────────
# PAM's default module search path is multiarch-specific
# (e.g. /lib/x86_64-linux-gnu/security/) — a module dropped at /lib/security/
# is silently unloadable on Debian/Ubuntu.
lh_module_path() {
    if [[ -n "${LH_MODULE_PATH_OVERRIDE:-}" ]]; then
        echo "$LH_MODULE_PATH_OVERRIDE"
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

# ── "Already fully configured?" short-circuit ────────────────────────────────
# Services covered by automatic activation: sudo family + polkit-1. Screenlock
# unlocking is handled entirely by hello-daemon's own watcher (polls
# org.freedesktop.ScreenSaver, unlocks via `loginctl unlock-session` on a face
# match — see hello_daemon/src/screenlock.rs) and never goes through PAM, so
# there's no screenlock PAM service to configure here. sddm is deliberately
# excluded too — see linux-hello-pam-autoconfigure.
lh_all_configured() {
    local svc
    for svc in sudo sudo-i su su-l polkit-1; do
        [[ -f "$PAM_DIR/$svc" ]] || continue
        grep -q "pam_linux_hello" "$PAM_DIR/$svc" || return 1
    done
    return 0
}

# ── Idempotent, validated insertion ───────────────────────────────────────────
# lh_configure_service <service> <context> <anchor_regex>
#
# Inserts the linux-hello auth line into /etc/pam.d/<service>, before the
# first line matching <anchor_regex> (falling back to the first "auth" line
# if the anchor isn't found). Makes a timestamped backup first. Skips
# services that are missing or already configured.
#
# After writing, validates the file wasn't corrupted (exactly one
# pam_linux_hello line added, file didn't shrink) and restores the backup if
# it was — the unattended timer has no human watching its output, so a
# malformed pam.d file must be caught and reverted automatically rather than
# left broken.
#
# <extra_opts>, if given, is appended verbatim after context=<context> on the
# generated auth line (e.g. "confirm", to require an explicit [y/N] before
# granting access — used for sudo/su, not for screenlock/sddm/polkit).
lh_configure_service() {
    local svc="$1" context="$2" anchor_re="$3" extra_opts="${4:-}"
    local file="$PAM_DIR/$svc"

    if [[ ! -f "$file" ]]; then
        echo "Service $svc: file missing, skipped"
        return 0
    fi

    if grep -q "pam_linux_hello" "$file"; then
        echo "Service $svc: already configured (skipped)"
        return 0
    fi

    local backup="$file.pre-linuxhello-$LH_TIMESTAMP"
    cp -p "$file" "$backup"
    local orig_lines
    orig_lines=$(wc -l < "$file")

    local lh_auth="${LH_LINE_TEMPLATE//%CONTEXT%/$context}"
    if [[ -n "$extra_opts" ]]; then
        lh_auth="$lh_auth $extra_opts"
    fi
    python3 - "$file" "$anchor_re" "$lh_auth" "$LH_MARKER_START" "$LH_MARKER_END" << 'PYEOF'
import sys, re

fpath, anchor_re, lh_auth, ms, me = sys.argv[1:]
with open(fpath) as f:
    lines = f.readlines()

def insert_at(lines, predicate):
    out = []
    done = False
    for line in lines:
        if not done and predicate(line):
            out.extend([ms + "\n", lh_auth + "\n", me + "\n"])
            done = True
        out.append(line)
    return out, done

out, done = insert_at(lines, lambda l: re.search(anchor_re, l))
if not done:
    out, done = insert_at(lines, lambda l: re.search(r'^auth\s', l))

if done:
    with open(fpath, "w") as f:
        f.writelines(out)
PYEOF

    # Post-write validation.
    local ok=1 hello_count new_lines
    grep -q "pam_linux_hello" "$file" || ok=0
    hello_count=$(grep -c "pam_linux_hello" "$file")
    [[ "$hello_count" -eq 1 ]] || ok=0
    new_lines=$(wc -l < "$file")
    [[ "$new_lines" -ge "$orig_lines" ]] || ok=0

    if [[ "$ok" -eq 1 ]]; then
        echo "Service $svc: linux-hello configured (context=$context)"
        return 0
    else
        cp -p "$backup" "$file"
        echo "Service $svc: configuration failed validation, restored from backup" >&2
        return 1
    fi
}

# ── SDDM (login screen) enable/disable ───────────────────────────────────────
# Deliberately separate from lh_configure_service's sudo/su/polkit-1 callers:
# SDDM starts hello-daemon-system.service, a root-owned, always-on,
# pre-authentication-reachable listener — explicit opt-in only, never part of
# automatic activation or the bare `install-pam.sh` default flow.
#
# lh_sddm_enable <sddm_qml_source>
# <sddm_qml_source> is the patched Login.qml to divert-install into the
# configured greeter theme (caller resolves the packaged-vs-checkout path).
lh_sddm_enable() {
    local sddm_qml_source="$1"

    lh_configure_service "sddm" "sddm" "@include common-auth"
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

# lh_sddm_disable — reverses lh_sddm_enable. Leaves sudo/su/polkit alone.
lh_sddm_disable() {
    local f="$PAM_DIR/sddm"
    local latest_bak
    latest_bak=$(find "$PAM_DIR" -maxdepth 1 -name "sddm.pre-linuxhello-*" -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2- || true)
    if [[ -n "$latest_bak" ]]; then
        cp "$latest_bak" "$f"
        echo "Restored: sddm ← $latest_bak"
    elif [[ -f "$f" ]]; then
        sed -i "/$LH_MARKER_START/,/$LH_MARKER_END/d" "$f"
        sed -i '/pam_linux_hello/d' "$f"
        echo "Cleaned: sddm (linux-hello lines removed)"
    fi

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
