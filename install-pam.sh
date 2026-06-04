#!/bin/bash
# Installation du module PAM Linux Hello
# À exécuter avec sudo
#
# Usage:
#   sudo ./install-pam.sh            # installer
#   sudo ./install-pam.sh --remove   # désactiver et restaurer les sauvegardes
#   sudo ./install-pam.sh --status   # afficher l'état actuel
#
# SÉCURITÉ : Ce script utilise toujours "auth sufficient" — le mot de passe
# reste TOUJOURS disponible comme fallback. Vous ne pouvez pas être bloqué.
# En cas de problème : sudo ./install-pam.sh --remove

set -euo pipefail

# ── Constantes ──────────────────────────────────────────────────────────────
SOURCE_SO="$(dirname "$(readlink -f "$0")")/target/release/libpam_linux_hello.so"
# Détecter l'architecture dynamiquement pour supporter x86_64, arm64, etc.
_MULTIARCH="$(dpkg-architecture -qDEB_HOST_MULTIARCH 2>/dev/null || gcc -dumpmachine 2>/dev/null || echo "x86_64-linux-gnu")"
PAM_MODULE="/lib/${_MULTIARCH}/security/pam_linux_hello.so"
PAM_DIR="/etc/pam.d"
TIMESTAMP="$(date +%s)"
# Ligne linux-hello à insérer (avec suffisant = fallback mot de passe garanti)
LH_LINE="auth       sufficient   pam_linux_hello.so context=%CONTEXT%"
# Marqueurs pour identifier les blocs ajoutés par ce script
MARKER_START="# >>> linux-hello-start"
MARKER_END="# <<< linux-hello-end"

# ── Couleurs ─────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
ok()   { echo -e "${GREEN}✓${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC}  $*"; }
err()  { echo -e "${RED}✗${NC} $*"; }

# ── Vérification root ────────────────────────────────────────────────────────
if [[ "$EUID" -ne 0 ]]; then
    err "Ce script doit être exécuté avec sudo"
    exit 1
fi

# ═══════════════════════════════════════════════════════════════════════════
# MODE --status
# ═══════════════════════════════════════════════════════════════════════════
if [[ "${1:-}" == "--status" ]]; then
    echo "=== État du module PAM linux-hello ==="
    if [[ -f "$PAM_MODULE" ]]; then
        ok "Module installé : $PAM_MODULE"
    else
        err "Module absent : $PAM_MODULE"
    fi
    for svc in sudo sudo-i su su-l sddm kde-screenlocker polkit-1; do
        f="$PAM_DIR/$svc"
        if [[ -f "$f" ]]; then
            if grep -q "pam_linux_hello" "$f" 2>/dev/null; then
                ok "$svc : linux-hello activé"
            else
                warn "$svc : linux-hello NON configuré"
            fi
        else
            echo "   $svc : fichier absent (normal pour polkit-1)"
        fi
    done
    exit 0
fi

# ═══════════════════════════════════════════════════════════════════════════
# MODE --remove  (désactivation + restauration des backups)
# ═══════════════════════════════════════════════════════════════════════════
if [[ "${1:-}" == "--remove" ]]; then
    echo "=== Désactivation de Linux Hello PAM ==="
    for svc in sudo sudo-i su su-l sddm polkit-1; do
        f="$PAM_DIR/$svc"
        # Chercher le backup le plus récent pour ce service
        latest_bak=$(find "$PAM_DIR" -maxdepth 1 -name "$svc.pre-linuxhello-*" -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2- || true)
        if [[ -n "$latest_bak" ]]; then
            cp "$latest_bak" "$f"
            ok "Restauré : $svc ← $latest_bak"
        elif [[ -f "$f" ]]; then
            # Pas de backup : supprimer les lignes linux-hello via sed
            sed -i "/$MARKER_START/,/$MARKER_END/d" "$f"
            sed -i '/pam_linux_hello/d' "$f"
            ok "Nettoyé : $svc (lignes linux-hello supprimées)"
        fi
    done
    # polkit-1 : s'il a été créé par ce script, le supprimer
    if grep -q "linux-hello" "$PAM_DIR/polkit-1" 2>/dev/null; then
        rm -f "$PAM_DIR/polkit-1"
        ok "Supprimé : polkit-1 (créé par ce script)"
    fi
    # kde-screenlocker n'est pas touché par --remove (déjà configuré manuellement)
    warn "kde-screenlocker laissé intact (configuré séparément)"
    echo ""
    ok "Linux Hello désactivé. Le mot de passe reprend le contrôle."
    exit 0
fi

# ═══════════════════════════════════════════════════════════════════════════
# MODE installation (défaut)
# ═══════════════════════════════════════════════════════════════════════════
echo "=== Installation du module PAM Linux Hello ==="
echo ""
echo "SÉCURITÉ : auth sufficient est utilisé partout."
echo "Le mot de passe reste TOUJOURS disponible comme fallback."
echo "En cas de problème : sudo $0 --remove"
echo ""

# ── 1. Installer le .so ──────────────────────────────────────────────────────
if [[ ! -f "$SOURCE_SO" ]]; then
    err "Module non compilé : $SOURCE_SO"
    echo "Compilez d'abord : cargo build --release"
    exit 1
fi
cp "$SOURCE_SO" "$PAM_MODULE"
chmod 644 "$PAM_MODULE"
ok "Module PAM installé : $PAM_MODULE"

# ── Fonction d'insertion safe ─────────────────────────────────────────────
# insert_before_pattern <fichier> <pattern_anchor> <ligne_à_insérer>
# Insère la ligne AVANT la première occurrence de pattern_anchor
insert_before_pattern() {
    local file="$1"
    local pattern="$2"
    local line_to_insert="$3"

    # Vérifier que l'anchor existe
    if ! grep -qE "$pattern" "$file"; then
        warn "Pattern '$pattern' introuvable dans $file — insertion ignorée"
        return 1
    fi
    # Insérer avant la première occurrence (sed : trouver la première ligne matching)
    sed -i "0,/$pattern/{s|$pattern|$line_to_insert\n&|}" "$file"
    return 0
}

# ── Fonction principale de configuration d'un service ────────────────────────
configure_service() {
    local svc="$1"
    local context="$2"
    local anchor="$3"  # pattern avant lequel insérer
    local file="$PAM_DIR/$svc"

    if [[ ! -f "$file" ]]; then
        warn "Service $svc : fichier absent, ignoré"
        return
    fi

    # Déjà configuré ?
    if grep -q "pam_linux_hello" "$file"; then
        ok "Service $svc : déjà configuré (ignoré)"
        return
    fi

    # Backup horodaté
    cp "$file" "$file.pre-linuxhello-$TIMESTAMP"
    ok "Backup : $file.pre-linuxhello-$TIMESTAMP"

    # Construire la ligne à insérer
    local lh_auth="${LH_LINE//%CONTEXT%/$context}"

    # Insérer avant l'anchor
    if insert_before_pattern "$file" "$anchor" "$MARKER_START"; then
        # Remplacer le marqueur seul par le bloc complet
        sed -i "s|$MARKER_START|$MARKER_START\n$lh_auth\n$MARKER_END|" "$file"
        # Nettoyer la ligne du marqueur en double qu'on vient de créer
        # (insert_before_pattern a inséré MARKER_START, puis on a remplacé par bloc)
        # → simplifier : réécrire proprement via python
        python3 -c "
import re, sys
with open('$file', 'r') as f:
    content = f.read()
# Supprimer les doublons créés par la double substitution
content = re.sub(r'$MARKER_START\n$MARKER_START\n', '$MARKER_START\n', content)
content = re.sub(r'$MARKER_END\n$MARKER_END\n', '$MARKER_END\n', content)
with open('$file', 'w') as f:
    f.write(content)
" 2>/dev/null || true
        ok "Service $svc : linux-hello configuré (context=$context)"
    else
        # Restaurer le backup si l'insertion échoue
        cp "$file.pre-linuxhello-$TIMESTAMP" "$file"
        warn "Service $svc : configuration ignorée (anchor non trouvé)"
    fi
}

# ── Fonction plus simple et robuste via Python ────────────────────────────────
configure_service_py() {
    local svc="$1"
    local context="$2"
    local anchor_re="$3"  # regex Python
    local file="$PAM_DIR/$svc"

    if [[ ! -f "$file" ]]; then
        warn "Service $svc : fichier absent, ignoré"
        return
    fi

    if grep -q "pam_linux_hello" "$file"; then
        ok "Service $svc : déjà configuré (ignoré)"
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
    # fallback : insérer au début des lignes auth
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

    ok "Service $svc : linux-hello configuré (context=$context)"
}

# ── 2. sudo et sudo-i ────────────────────────────────────────────────────────
# sudo utilise @include common-auth → insérer AVANT pour avoir la biométrie en premier
configure_service_py "sudo"   "sudo"   "@include common-auth"
configure_service_py "sudo-i" "sudo"   "@include common-auth"

# ── 3. su et su-l ────────────────────────────────────────────────────────────
configure_service_py "su"     "sudo"   "@include common-auth"
configure_service_py "su-l"   "sudo"   "@include common-auth"

# ── 4. SDDM (écran de connexion) ─────────────────────────────────────────────
# Insérer avant @include common-auth (après les checks nologin/pam_succeed_if)
configure_service_py "sddm"   "sddm"   "@include common-auth"

# ── 5. polkit-1 (dialogues graphiques "Authentification requise") ─────────────
POLKIT_FILE="$PAM_DIR/polkit-1"
if [[ ! -f "$POLKIT_FILE" ]]; then
    cat > "$POLKIT_FILE" << 'EOF'
#%PAM-1.0
# Linux Hello - Authentification pour les dialogues polkit (pkexec, élévation graphique)
# auth sufficient = si biométrie OK → accès accordé ; sinon → fallback mot de passe

auth       sufficient   pam_linux_hello.so context=polkit
auth       required     pam_unix.so nullok
@include common-account
EOF
    ok "Service polkit-1 : créé avec linux-hello + fallback mot de passe"
else
    configure_service_py "polkit-1" "polkit" "^auth"
fi

# ── 6. kde-screenlocker (déjà configuré, vérification) ───────────────────────
if grep -q "pam_linux_hello" "$PAM_DIR/kde-screenlocker" 2>/dev/null; then
    ok "Service kde-screenlocker : déjà configuré ✓"
else
    configure_service_py "kde-screenlocker" "screenlock" "^auth"
fi

# ── Résumé ───────────────────────────────────────────────────────────────────
echo ""
echo "=== Résumé de configuration ==="
for svc in sudo sudo-i su su-l sddm polkit-1 kde-screenlocker; do
    f="$PAM_DIR/$svc"
    if [[ -f "$f" ]]; then
        if grep -q "pam_linux_hello" "$f"; then
            ok "$svc : linux-hello actif"
            grep "pam_linux_hello" "$f" | sed 's/^/     /'
        else
            warn "$svc : non configuré"
        fi
    fi
done

echo ""
echo "=== Test rapide ==="
echo "Testez avec : sudo -k && sudo ls /"
echo "Si bloqué    : sudo $0 --remove"
echo ""
ok "Installation terminée."
