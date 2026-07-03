#!/bin/bash
# Aperçu du projet Linux Hello

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║          Linux Hello - Authentification Faciale Linux          ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""

cd /home/edtech/Documents/linux-hello-rust || exit 1

# Afficher la structure du projet
echo "📁 Structure du Projet:"
echo ""
find . -maxdepth 2 -type d -not -path '*/target/*' -not -path '*/.git/*' | head -20 | sed 's/^\.\//   /'
echo ""

# Compiler
echo "🔨 Compilation..."
cargo build --release 2>&1 | grep -E "Compiling|Finished"
echo ""

# Afficher les artefacts compilés
echo "📦 Artefacts:"
find target/release -maxdepth 1 \( -name "hello-daemon" -o -name "linux-hello" -o -name "libpam_linux_hello.so*" \) -type f 2>/dev/null | while read -r file; do
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    size_h=$(numfmt --to=iec-i --suffix=B "$size" 2>/dev/null || printf "%s\n" "$size")
    printf "   %s (%s)\n" "$file" "$size_h"
done
echo ""

# Afficher la config
echo "⚙️  Configuration par défaut:"
echo "   Stockage: ~/.local/share/linux-hello"
echo "   Service D-Bus: com.linuxhello.FaceAuth"
echo "   Seuil similarité: 0.6"
echo ""

# Afficher les tests disponibles
echo "✅ Tests disponibles:"
echo ""
echo "   Tests de daemon:"
echo "      ./test-pam-full.sh    - Test complet daemon+D-Bus"
echo ""
echo "   Tests d'intégration PAM:"
echo "      ./test-sudo.sh        - Test avec sudo"
echo "      ./test-screenlock.sh  - Test avec screenlock"
echo ""
echo "   Préparation:"
echo "      ./prepare-pam-test.sh - Enregistrer un visage"
echo ""

# Afficher les commandes utiles
echo "🚀 Commandes Utiles:"
echo ""
echo "   # Démarrer le daemon"
echo "   ./target/release/hello-daemon --debug"
echo ""
echo "   # Enregistrer un visage"
echo "   dbus-send --session --print-reply \\"
echo "     --dest=com.linuxhello.FaceAuth \\"
echo "     /com/linuxhello/FaceAuth \\"
echo "     com.linuxhello.FaceAuth.RegisterFace \\"
echo "     string:'{\"user_id\":1000,\"context\":\"test\",\"timeout_ms\":5000,\"num_samples\":1}'"
echo ""
echo "   # Vérifier un visage"
echo "   dbus-send --session --print-reply \\"
echo "     --dest=com.linuxhello.FaceAuth \\"
echo "     /com/linuxhello/FaceAuth \\"
echo "     com.linuxhello.FaceAuth.Verify \\"
echo "     string:'{\"user_id\":1000,\"context\":\"test\",\"timeout_ms\":3000}'"
echo ""

# Afficher la documentation
echo "📚 Documentation:"
echo ""
echo "   README.md                   - Vue d'ensemble"
echo "   docs/PAM_MODULE.md          - Documentation module PAM"
echo "   docs/INTEGRATION_GUIDE.md   - Guide d'intégration sudo/screenlock"
echo ""

# Afficher les fichiers de configuration
echo "⚙️  Configurations PAM:"
echo ""
for f in sudo-linux-hello.pam kde-screenlock-linux-hello.pam test-pam-config; do
    if [ -f "$f" ]; then
        echo "   ✓ $f"
    else
        echo "   ✗ $f"
    fi
done
echo ""

# Afficher le statut du daemon
echo "📡 Statut Runtime:"
if dbus-send --session --print-reply --dest=com.linuxhello.FaceAuth /com/linuxhello/FaceAuth com.linuxhello.FaceAuth.Ping 2>/dev/null | grep -q "pong"; then
    echo "   ✓ Daemon D-Bus: Actif"
else
    echo "   ✗ Daemon D-Bus: Inactif (lancez: ./target/release/hello-daemon)"
fi
echo ""

# Afficher les prochaines étapes
echo "📋 Prochaines Étapes:"
echo ""
echo "   1. Test du daemon:"
echo "      ./target/release/hello-daemon &"
echo "      ./prepare-pam-test.sh"
echo ""
echo "   2. Test PAM avec sudo:"
echo "      ./test-sudo.sh"
echo ""
echo "   3. Installation système:"
echo "      sudo install -m 644 target/release/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/"
echo "      sudo nano /etc/pam.d/sudo  # ajouter les lignes linux-hello"
echo ""
echo "   4. Configuration daemon au démarrage:"
echo "      mkdir -p ~/.config/systemd/user"
echo "      # Voir docs/INTEGRATION_GUIDE.md pour détails"
echo ""
echo "   5. Implémenter vraie caméra:"
echo "      Voir hello_camera/src/lib.rs"
echo ""

echo "═══════════════════════════════════════════════════════════════════"
echo ""
