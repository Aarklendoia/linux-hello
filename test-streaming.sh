#!/bin/bash
# Test d'intégration simple pour la méthode start_capture_stream D-Bus
# 
# Usage:
#   ./test-streaming.sh
#
# Ceci va:
# 1. Lancer le daemon hello_daemon
# 2. Appeler la méthode D-Bus start_capture_stream
# 3. Observer les signaux (avec dbus-monitor si disponible)

set -e

DAEMON_BIN="${DAEMON_BIN:-./target/release/hello-daemon}"
USER_ID=1000
NUM_FRAMES=5
TIMEOUT_MS=10000

echo "=========================================="
echo "Test Streaming Capture D-Bus"
echo "=========================================="
echo ""
echo "Configuration:"
echo "  - Daemon: $DAEMON_BIN"
echo "  - User ID: $USER_ID"
echo "  - Frames: $NUM_FRAMES"
echo "  - Timeout: ${TIMEOUT_MS}ms"
echo ""

# Vérifier que le daemon est compilé
if [ ! -f "$DAEMON_BIN" ]; then
    echo "❌ Daemon non trouvé: $DAEMON_BIN"
    echo "   Veuillez d'abord compiler avec: cargo build"
    exit 1
fi

# Lancer le daemon en arrière-plan
echo "✓ Lancement du daemon..."
$DAEMON_BIN &
DAEMON_PID=$!
echo "  PID: $DAEMON_PID"

# Attendre que le daemon se lance (D-Bus connexion)
sleep 3

# Nettoyer si interruption
cleanup() {
    echo ""
    echo "Arrêt du daemon (PID=$DAEMON_PID)..."
    kill $DAEMON_PID 2>/dev/null || true
    wait $DAEMON_PID 2>/dev/null || true
    echo "✓ Nettoyage terminé"
}
trap cleanup EXIT INT TERM

echo ""
echo "✓ Daemon lancé avec succès"
echo ""

# Vérifier que le daemon répond
echo "✓ Test de connexion D-Bus..."
if busctl call com.linuxhello.FaceAuth /com/linuxhello/FaceAuth \
    com.linuxhello.FaceAuth Ping >/dev/null 2>&1; then
    echo "  ✓ Daemon répond au Ping"
else
    echo "  ❌ Daemon ne répond pas"
    exit 1
fi

echo ""
echo "✓ Appel de start_capture_stream..."
echo "  Paramètres: user_id=$USER_ID, num_frames=$NUM_FRAMES, timeout_ms=$TIMEOUT_MS"
echo ""

# Appeler la méthode D-Bus
RESULT=$(busctl call com.linuxhello.FaceAuth /com/linuxhello/FaceAuth \
    com.linuxhello.FaceAuth StartCaptureStream uuu \
    $USER_ID $NUM_FRAMES $TIMEOUT_MS 2>&1) || {
    echo "❌ Erreur lors de l'appel D-Bus"
    echo "Résultat: $RESULT"
    exit 1
}

echo "✓ Résultat: $RESULT"
echo ""

echo "=========================================="
echo "✓ Test réussi!"
echo "=========================================="
echo ""
echo "Pour observer les signaux D-Bus, exécutez:"
echo "  dbus-monitor --session"
echo ""
