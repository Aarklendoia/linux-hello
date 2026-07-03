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

# ── Screenlock service name ───────────────────────────────────────────────────
# KDE's screenlock PAM service is named "kde-screenlocker" on current Kubuntu,
# but older/other setups (and this project's own preinst/postrm/docs) use "kde".
# Try the more specific name first.
lh_screenlock_file() {
    if [[ -f "$PAM_DIR/kde-screenlocker" ]]; then
        echo "kde-screenlocker"
    elif [[ -f "$PAM_DIR/kde" ]]; then
        echo "kde"
    fi
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
# Services covered by automatic activation: sudo family + polkit-1 + screenlock.
# sddm is deliberately excluded — see linux-hello-pam-autoconfigure.
lh_all_configured() {
    local svc screenlock_file
    for svc in sudo sudo-i su su-l polkit-1; do
        [[ -f "$PAM_DIR/$svc" ]] || continue
        grep -q "pam_linux_hello" "$PAM_DIR/$svc" || return 1
    done
    screenlock_file="$(lh_screenlock_file)"
    if [[ -n "$screenlock_file" ]]; then
        grep -q "pam_linux_hello" "$PAM_DIR/$screenlock_file" || return 1
    fi
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
lh_configure_service() {
    local svc="$1" context="$2" anchor_re="$3"
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
