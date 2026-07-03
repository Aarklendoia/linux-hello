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

set -euo pipefail

# ── Constants ────────────────────────────────────────────────────────────────
SOURCE_SO="$(dirname "$(readlink -f "$0")")/target/release/libpam_linux_hello.so"
# Detect architecture dynamically to support x86_64, arm64, etc.
_MULTIARCH="$(dpkg-architecture -qDEB_HOST_MULTIARCH 2>/dev/null || gcc -dumpmachine 2>/dev/null || echo "x86_64-linux-gnu")"
PAM_MODULE="/lib/${_MULTIARCH}/security/pam_linux_hello.so"
PAM_DIR="/etc/pam.d"
TIMESTAMP="$(date +%s)"
# linux-hello line to insert (with sufficient = guaranteed password fallback)
LH_LINE="auth       sufficient   pam_linux_hello.so context=%CONTEXT%"
# Markers to identify the blocks added by this script
MARKER_START="# >>> linux-hello-start"
MARKER_END="# <<< linux-hello-end"

# ── Colors ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
ok()   { echo -e "${GREEN}✓${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC}  $*"; }
err()  { echo -e "${RED}✗${NC} $*"; }

# ── Root check ───────────────────────────────────────────────────────────────
if [[ "$EUID" -ne 0 ]]; then
    err "This script must be run with sudo"
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
    for svc in sudo sudo-i su su-l sddm kde-screenlocker polkit-1; do
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
    exit 0
fi

# ═══════════════════════════════════════════════════════════════════════════
# MODE --remove  (disable + restore backups)
# ═══════════════════════════════════════════════════════════════════════════
if [[ "${1:-}" == "--remove" ]]; then
    echo "=== Disabling Linux Hello PAM ==="
    for svc in sudo sudo-i su su-l sddm polkit-1; do
        f="$PAM_DIR/$svc"
        # Look for the most recent backup for this service
        latest_bak=$(find "$PAM_DIR" -maxdepth 1 -name "$svc.pre-linuxhello-*" -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2- || true)
        if [[ -n "$latest_bak" ]]; then
            cp "$latest_bak" "$f"
            ok "Restored: $svc ← $latest_bak"
        elif [[ -f "$f" ]]; then
            # No backup: remove the linux-hello lines via sed
            sed -i "/$MARKER_START/,/$MARKER_END/d" "$f"
            sed -i '/pam_linux_hello/d' "$f"
            ok "Cleaned: $svc (linux-hello lines removed)"
        fi
    done
    # polkit-1: if it was created by this script, remove it
    if grep -q "linux-hello" "$PAM_DIR/polkit-1" 2>/dev/null; then
        rm -f "$PAM_DIR/polkit-1"
        ok "Removed: polkit-1 (created by this script)"
    fi
    # kde-screenlocker is not touched by --remove (already configured manually)
    warn "kde-screenlocker left untouched (configured separately)"
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
cp "$SOURCE_SO" "$PAM_MODULE"
chmod 644 "$PAM_MODULE"
ok "PAM module installed: $PAM_MODULE"

# ── Safe insertion function ───────────────────────────────────────────────
# insert_before_pattern <file> <pattern_anchor> <line_to_insert>
# Inserts the line BEFORE the first occurrence of pattern_anchor
insert_before_pattern() {
    local file="$1"
    local pattern="$2"
    local line_to_insert="$3"

    # Check that the anchor exists
    if ! grep -qE "$pattern" "$file"; then
        warn "Pattern '$pattern' not found in $file — insertion skipped"
        return 1
    fi
    # Insert before the first occurrence (sed: find the first matching line)
    sed -i "0,/$pattern/{s|$pattern|$line_to_insert\n&|}" "$file"
    return 0
}

# ── Main function to configure a service ──────────────────────────────────────
configure_service() {
    local svc="$1"
    local context="$2"
    local anchor="$3"  # pattern before which to insert
    local file="$PAM_DIR/$svc"

    if [[ ! -f "$file" ]]; then
        warn "Service $svc: file missing, skipped"
        return
    fi

    # Already configured?
    if grep -q "pam_linux_hello" "$file"; then
        ok "Service $svc: already configured (skipped)"
        return
    fi

    # Timestamped backup
    cp "$file" "$file.pre-linuxhello-$TIMESTAMP"
    ok "Backup: $file.pre-linuxhello-$TIMESTAMP"

    # Build the line to insert
    local lh_auth="${LH_LINE//%CONTEXT%/$context}"

    # Insert before the anchor
    if insert_before_pattern "$file" "$anchor" "$MARKER_START"; then
        # Replace the lone marker with the full block
        sed -i "s|$MARKER_START|$MARKER_START\n$lh_auth\n$MARKER_END|" "$file"
        # Clean up the duplicate marker line we just created
        # (insert_before_pattern inserted MARKER_START, then we replaced it with the block)
        # → simplify: rewrite cleanly via python
        python3 -c "
import re, sys
with open('$file', 'r') as f:
    content = f.read()
# Remove duplicates created by the double substitution
content = re.sub(r'$MARKER_START\n$MARKER_START\n', '$MARKER_START\n', content)
content = re.sub(r'$MARKER_END\n$MARKER_END\n', '$MARKER_END\n', content)
with open('$file', 'w') as f:
    f.write(content)
" 2>/dev/null || true
        ok "Service $svc: linux-hello configured (context=$context)"
    else
        # Restore the backup if insertion fails
        cp "$file.pre-linuxhello-$TIMESTAMP" "$file"
        warn "Service $svc: configuration skipped (anchor not found)"
    fi
}

# ── Simpler, more robust function via Python ──────────────────────────────────
configure_service_py() {
    local svc="$1"
    local context="$2"
    local anchor_re="$3"  # Python regex
    local file="$PAM_DIR/$svc"

    if [[ ! -f "$file" ]]; then
        warn "Service $svc: file missing, skipped"
        return
    fi

    if grep -q "pam_linux_hello" "$file"; then
        ok "Service $svc: already configured (skipped)"
        return
    fi

    cp "$file" "$file.pre-linuxhello-$TIMESTAMP"

    local lh_auth="${LH_LINE//%CONTEXT%/$context}"
    python3 - "$file" "$anchor_re" "$lh_auth" "$MARKER_START" "$MARKER_END" << 'PYEOF'
import sys, re
fpath, anchor_re, lh_auth, ms, me = sys.argv[1:]
with open(fpath) as f:
    lines = f.readlines()
inserted = False
out = []
for line in lines:
    if not inserted and re.search(anchor_re, line):
        out.append(ms + "\n")
        out.append(lh_auth + "\n")
        out.append(me + "\n")
        inserted = True
    out.append(line)
if not inserted:
    # fallback: insert at the start of the auth lines
    out2 = []
    inserted2 = False
    for line in out:
        if not inserted2 and re.search(r'^auth\s', line):
            out2.append(ms + "\n")
            out2.append(lh_auth + "\n")
            out2.append(me + "\n")
            inserted2 = True
        out2.append(line)
    out = out2
with open(fpath, "w") as f:
    f.writelines(out)
PYEOF

    ok "Service $svc: linux-hello configured (context=$context)"
}

# ── 2. sudo and sudo-i ───────────────────────────────────────────────────────
# sudo uses @include common-auth → insert BEFORE it so biometrics run first
configure_service_py "sudo"   "sudo"   "@include common-auth"
configure_service_py "sudo-i" "sudo"   "@include common-auth"

# ── 3. su and su-l ───────────────────────────────────────────────────────────
configure_service_py "su"     "sudo"   "@include common-auth"
configure_service_py "su-l"   "sudo"   "@include common-auth"

# ── 4. SDDM (login screen) ───────────────────────────────────────────────────
# Insert before @include common-auth (after the nologin/pam_succeed_if checks)
configure_service_py "sddm"   "sddm"   "@include common-auth"

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
    configure_service_py "polkit-1" "polkit" "^auth"
fi

# ── 6. kde-screenlocker (already configured, verification) ───────────────────
if grep -q "pam_linux_hello" "$PAM_DIR/kde-screenlocker" 2>/dev/null; then
    ok "Service kde-screenlocker: already configured ✓"
else
    configure_service_py "kde-screenlocker" "screenlock" "^auth"
fi

# ── Summary ──────────────────────────────────────────────────────────────────
echo ""
echo "=== Configuration summary ==="
for svc in sudo sudo-i su su-l sddm polkit-1 kde-screenlocker; do
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

echo ""
echo "=== Quick test ==="
echo "Test with: sudo -k && sudo ls /"
echo "If locked out: sudo $0 --remove"
echo ""
ok "Installation complete."
