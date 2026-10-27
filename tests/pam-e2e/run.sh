#!/bin/bash
# End-to-end PAM tests (#237): the built pam_linux_hello.so, loaded by the real
# Linux-PAM through the real pam-auth-update profile, driven by pamtester.
#
# Runs as root inside a throwaway Ubuntu container: it installs packages and
# rewrites /etc/pam.d. Never run it on a real machine.
#
#   docker run --rm -v "$PWD:/src:ro" ubuntu:resolute \
#       bash /src/tests/pam-e2e/run.sh /src/target/ci/libpam_linux_hello.so
#
# The daemon is replaced by tests/pam-e2e/fake_helper.py, so camera, matching
# and TPM aren't covered here: what is covered is everything between PAM and
# the helper socket (service -> context mapping, confirmation, fallback to
# pam_unix, the SDDM marker, the system socket, the legacy migration).
set -euo pipefail

SO="$1"
SRC="$(cd "$(dirname "$0")/../.." && pwd)"
HELPER="$SRC/tests/pam-e2e/fake_helper.py"
USER_NAME=alice
USER_PASS=correct-horse
WORK=/tmp/pam-e2e

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends pamtester sudo python3 libpam-runtime util-linux > /dev/null

# ── Install the module and the profile, as libpam-linux-hello does ──────────
SECDIR="$(dirname "$(find /lib /usr/lib -name pam_unix.so -print -quit)")"
install -m 0644 "$SO" "$SECDIR/pam_linux_hello.so"
install -m 0644 "$SRC/debian/pam-configs/linux-hello" /usr/share/pam-configs/linux-hello
# shellcheck source=pam-lib.sh
source "$SRC/pam-lib.sh"

useradd -m "$USER_NAME"
echo "$USER_NAME:$USER_PASS" | chpasswd
USER_UID="$(id -u "$USER_NAME")"
mkdir -p /run/hello-pam "$WORK"
chmod 1777 /run/hello-pam "$WORK"

# Neither sddm nor polkitd is installed: give both services the stack their
# packages ship (common-auth is where the profile's line lives).
printf '@include common-auth\n@include common-account\n' > /etc/pam.d/sddm
printf '@include common-auth\n@include common-account\n' > /etc/pam.d/polkit-1

FAILURES=0
pass() { echo "  ok    $1"; }
fail() { echo "  FAIL  $1"; FAILURES=$((FAILURES + 1)); }

# ── 1. Migration from the pre-#215 per-service blocks ───────────────────────
echo "Legacy migration"
for svc in sudo su sddm; do
    sed -i "1i # >>> linux-hello-start\nauth       sufficient   pam_linux_hello.so context=sudo\n# <<< linux-hello-end" "/etc/pam.d/$svc"
done
rm -f "$LH_SDDM_MARKER"
lh_migrate_legacy_config > /dev/null
if grep -l pam_linux_hello /etc/pam.d/sudo /etc/pam.d/su /etc/pam.d/sddm 2> /dev/null; then
    fail "legacy lines removed from sudo/su/sddm"
else
    pass "legacy lines removed from sudo/su/sddm"
fi
if lh_profile_enabled; then pass "profile enabled in common-auth"; else fail "profile enabled in common-auth"; fi
if [[ -f "$LH_SDDM_MARKER" ]]; then pass "SDDM opt-in kept as the marker"; else fail "SDDM opt-in kept as the marker"; fi
rm -f "$LH_SDDM_MARKER"

lh_profile_enable > /dev/null
lh_profile_enabled || { echo "pam-auth-update did not enable the profile:"; cat /etc/pam.d/common-auth; exit 1; }

# ── Helpers ─────────────────────────────────────────────────────────────────
as_user() { setpriv --reuid="$USER_NAME" --regid="$USER_NAME" --init-groups -- "$@"; }

USER_SOCK="/run/hello-pam/$USER_UID.socket"
SYSTEM_SOCK=/run/hello-pam/system.socket
MODE="$WORK/mode"
LOG="$WORK/requests.log"
echo match > "$MODE"
: > "$LOG"
chmod 0666 "$MODE" "$LOG"

start_user_helper() {
    as_user python3 "$HELPER" "$USER_SOCK" "$MODE" "$LOG" &
    USER_HELPER_PID=$!
    for _ in $(seq 50); do [[ -S "$USER_SOCK" ]] && return; sleep 0.1; done
    echo "fake helper did not start"; exit 1
}
stop_user_helper() { kill "$USER_HELPER_PID" 2> /dev/null || true; wait "$USER_HELPER_PID" 2> /dev/null || true; rm -f "$USER_SOCK"; }

# scenario <name> <service> <mode> <stdin> <expected exit: 0|fail> <helper: contacted|silent> [<output must contain>]
scenario() {
    local name="$1" service="$2" mode="$3" input="$4" expect="$5" contact="$6" needle="${7:-}"
    local out status before after
    echo "$mode" > "$MODE"
    before=$(wc -l < "$LOG")
    set +e
    out=$(printf '%b' "$input" | as_user pamtester "$service" "$USER_NAME" authenticate 2>&1)
    status=$?
    set -e
    after=$(wc -l < "$LOG")

    local ok=1
    if [[ "$expect" == 0 ]]; then [[ $status -eq 0 ]] || ok=0; else [[ $status -ne 0 ]] || ok=0; fi
    if [[ "$contact" == contacted ]]; then [[ $after -gt $before ]] || ok=0; else [[ $after -eq $before ]] || ok=0; fi
    if [[ -n "$needle" ]] && ! grep -qF -- "$needle" <<< "$out"; then ok=0; fi

    if [[ $ok -eq 1 ]]; then
        pass "$name"
    else
        fail "$name (exit $status, helper requests $before -> $after)"
        while IFS= read -r line; do echo "          | $line"; done <<< "$out"
    fi
}

# ── 2. Per-user socket ──────────────────────────────────────────────────────
echo "sudo / su / polkit (per-user socket)"
start_user_helper
scenario "sudo: match + y, no password"            sudo     match   'y\n'                   0    contacted "Face recognized"
scenario "sudo: match + n, then password"          sudo     match   "n\n$USER_PASS\n"       0    contacted "Not confirmed"
scenario "sudo: match + n, wrong password"         sudo     match   'n\nwrong\n'            fail contacted
scenario "sudo: no match, then password"           sudo     nomatch "$USER_PASS\n"          0    contacted "Face not recognized"
scenario "sudo: no match, wrong password"          sudo     nomatch 'wrong\n'               fail contacted
scenario "sudo: no models, then password"          sudo     models  "$USER_PASS\n"          0    contacted
scenario "su: match + y, no password"              su       match   'y\n'                   0    contacted "Face recognized"
scenario "polkit-1: match, no confirmation"        polkit-1 match   ''                      0    contacted "Face recognized"
scenario "login: not handled, password only"       login    match   "$USER_PASS\n"          0    silent
scenario "sddm without marker: password only"      sddm     match   "$USER_PASS\n"          0    silent
if grep -q '"context":"sudo"' "$LOG" && grep -q '"context":"polkit"' "$LOG"; then
    pass "contexts sent: sudo, polkit"
else
    fail "contexts sent: sudo, polkit"; cat "$LOG"
fi
stop_user_helper
scenario "sudo: helper down, then password"        sudo     match   "$USER_PASS\n"          0    silent

# ── 3. SDDM: root-owned marker + system socket ──────────────────────────────
echo "sddm (system socket)"
lh_sddm_marker_set
python3 "$HELPER" "$SYSTEM_SOCK" "$MODE" "$LOG" &
SYSTEM_HELPER_PID=$!
for _ in $(seq 50); do [[ -S "$SYSTEM_SOCK" ]] && break; sleep 0.1; done
scenario "sddm with marker: match, no password"    sddm     match   ''                      0    contacted "Face recognized"
scenario "sddm with marker: no match, password"    sddm     nomatch "$USER_PASS\n"          0    contacted
kill "$SYSTEM_HELPER_PID" 2> /dev/null || true; wait "$SYSTEM_HELPER_PID" 2> /dev/null || true; rm -f "$SYSTEM_SOCK"

# A socket squatted by another uid must not be trusted (peer-uid check).
echo "untrusted socket"
as_user python3 "$HELPER" "$SYSTEM_SOCK" "$MODE" "$LOG" &
SQUAT_PID=$!
for _ in $(seq 50); do [[ -S "$SYSTEM_SOCK" ]] && break; sleep 0.1; done
scenario "sddm: socket owned by a user is refused" sddm     match   "$USER_PASS\n"          0    contacted
scenario "sddm: squatted socket grants nothing"   sddm     match   ''                      fail contacted
kill "$SQUAT_PID" 2> /dev/null || true; wait "$SQUAT_PID" 2> /dev/null || true
rm -f "$LH_SDDM_MARKER"

echo
if [[ $FAILURES -gt 0 ]]; then
    echo "$FAILURES scenario(s) failed"
    exit 1
fi
echo "All PAM end-to-end scenarios passed"
