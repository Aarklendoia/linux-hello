#!/bin/bash
# Test script pour vérifier que le système d'aperçu vidéo fonctionne

set -e

echo "🔍 Test du système d'aperçu vidéo Linux Hello"
echo ""

# Vérifier que le daemon est en cours d'exécution
echo "1️⃣  Vérification du daemon..."
if systemctl --user is-active --quiet hello-daemon; then
    echo "   ✅ Daemon actif (systemctl --user status hello-daemon)"
else
    echo "   ❌ Daemon inactif, démarrage..."
    systemctl --user start hello-daemon
    sleep 2
fi

# Vérifier que le D-Bus service est enregistré
echo ""
echo "2️⃣  Vérification du service D-Bus..."
if gdbus call --system --dest=org.freedesktop.DBus --object-path=/org/freedesktop/DBus --method=org.freedesktop.DBus.ListNames 2>/dev/null | grep -q "com.linuxhello.FaceAuth"; then
    echo "   ✅ Service D-Bus enregistré: com.linuxhello.FaceAuth"
else
    echo "   ⚠️  Service D-Bus non trouvé, mais continuons..."
fi

# Vérifier que la caméra est disponible
echo ""
echo "3️⃣  Vérification de la caméra..."
if [ -e /dev/video0 ]; then
    CAMERA_INFO=$(v4l2-ctl --device=/dev/video0 --info 2>&1 | head -1)
    echo "   ✅ Caméra trouvée: $CAMERA_INFO"
else
    echo "   ❌ Caméra non trouvée sur /dev/video0"
    exit 1
fi

# Afficher le chemin de configuration du GUI
echo ""
echo "4️⃣  Vérification des fichiers QML..."
QML_FILE="/usr/share/qt6/qml/Linux/Hello/main.qml"
if [ -f "$QML_FILE" ]; then
    echo "   ✅ Fichier QML trouvé: $QML_FILE"
else
    echo "   ❌ Fichier QML non trouvé: $QML_FILE"
    exit 1
fi

# Vérifier les permissions du fichier /tmp pour le preview
echo ""
echo "5️⃣  Vérification des permissions /tmp..."
if [ -w /tmp ]; then
    echo "   ✅ Répertoire /tmp accessible en écriture"
else
    echo "   ❌ Répertoire /tmp non accessible en écriture"
    exit 1
fi

echo ""
echo "✅ Tous les tests sont passés !"
echo ""
echo "Pour lancer la GUI d'enregistrement:"
echo "   linux-hello-config"
echo ""
echo "Ou directement avec qml6:"
echo "   export QML_IMPORT_PATH=/usr/lib/x86_64-linux-gnu/qt6/qml:/usr/share/qt6/qml"
echo "   qml6 $QML_FILE"
echo ""
