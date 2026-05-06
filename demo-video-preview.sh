#!/bin/bash
# Script de démonstration : capture vidéo en direct et affichage de l'aperçu

set -e

echo "🎬 Démonstration: Aperçu vidéo en direct Linux Hello"
echo ""

# Lancer le daemon si nécessaire
if ! systemctl --user is-active --quiet hello-daemon; then
    echo "▶️  Démarrage du daemon..."
    systemctl --user start hello-daemon
    sleep 2
fi

echo "📸 Appel de StartCaptureStream via D-Bus..."
echo "   user_id=1000, num_frames=100, timeout=15000ms"
echo ""

# Démarrer la capture vidéo
# Paramètres: user_id (uint32)=1000, num_frames (uint32)=100, timeout_ms (uint64)=15000
RESULT=$(dbus-send --print-reply \
    --session \
    --dest=com.linuxhello.FaceAuth \
    /com/linuxhello/FaceAuth \
    com.linuxhello.FaceAuth.StartCaptureStream \
    uint32:1000 \
    uint32:100 \
    uint64:15000 2>&1) || {
    echo "⚠️  Appel D-Bus échoué:"
    echo "$RESULT" | head -5
    echo ""
}

echo "$RESULT" | grep -E "string|OK" && echo "✅ Appel lancé avec succès" || echo "⚠️  Pas de confirmation"

echo ""
echo "⏳ Attente de quelques frames... (5 secondes)"
sleep 5

# Vérifier que le fichier preview a été créé
if [ -f /tmp/linux-hello-preview.jpg ]; then
    SIZE=$(du -h /tmp/linux-hello-preview.jpg | cut -f1)
    echo "✅ Fichier d'aperçu créé: /tmp/linux-hello-preview.jpg ($SIZE)"
    echo ""
    
    # Afficher les informations de l'image
    echo "📊 Détails de l'image:"
    file /tmp/linux-hello-preview.jpg || true
    identify /tmp/linux-hello-preview.jpg 2>/dev/null || echo "   (ImageMagick non installé)"
else
    echo "❌ Fichier d'aperçu non trouvé"
fi

echo ""
echo "▶️  Arrêt de la capture..."
dbus-send --print-reply \
    --session \
    --dest=com.linuxhello.FaceAuth \
    /com/linuxhello/FaceAuth \
    com.linuxhello.FaceAuth.StopCaptureStream 2>/dev/null || true

echo "✅ Démo terminée"
echo ""
echo "💡 Vous pouvez maintenant lancer la GUI:"
echo "   linux-hello-config"
echo ""
