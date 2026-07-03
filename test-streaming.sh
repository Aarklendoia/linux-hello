#!/bin/bash
# Simple integration test for the D-Bus start_capture_stream method
#
# Usage:
#   ./test-streaming.sh
#
# This will:
# 1. Launch the hello_daemon daemon
# 2. Call the D-Bus start_capture_stream method
# 3. Observe the signals (with dbus-monitor if available)

set -e

DAEMON_BIN="${DAEMON_BIN:-./target/release/hello-daemon}"
USER_ID=1000
NUM_FRAMES=5
TIMEOUT_MS=10000

echo "=========================================="
echo "D-Bus Streaming Capture Test"
echo "=========================================="
echo ""
echo "Configuration:"
echo "  - Daemon: $DAEMON_BIN"
echo "  - User ID: $USER_ID"
echo "  - Frames: $NUM_FRAMES"
echo "  - Timeout: ${TIMEOUT_MS}ms"
echo ""

# Check that the daemon is built
if [ ! -f "$DAEMON_BIN" ]; then
    echo "❌ Daemon not found: $DAEMON_BIN"
    echo "   Please build first with: cargo build"
    exit 1
fi

# Launch the daemon in the background
echo "✓ Launching the daemon..."
$DAEMON_BIN &
DAEMON_PID=$!
echo "  PID: $DAEMON_PID"

# Wait for the daemon to start up (D-Bus connection)
sleep 3

# Clean up on interruption
cleanup() {
    echo ""
    echo "Stopping the daemon (PID=$DAEMON_PID)..."
    kill $DAEMON_PID 2>/dev/null || true
    wait $DAEMON_PID 2>/dev/null || true
    echo "✓ Cleanup complete"
}
trap cleanup EXIT INT TERM

echo ""
echo "✓ Daemon launched successfully"
echo ""

# Check that the daemon responds
echo "✓ Testing D-Bus connection..."
if busctl call com.linuxhello.FaceAuth /com/linuxhello/FaceAuth \
    com.linuxhello.FaceAuth Ping >/dev/null 2>&1; then
    echo "  ✓ Daemon responds to Ping"
else
    echo "  ❌ Daemon is not responding"
    exit 1
fi

echo ""
echo "✓ Calling start_capture_stream..."
echo "  Parameters: user_id=$USER_ID, num_frames=$NUM_FRAMES, timeout_ms=$TIMEOUT_MS"
echo ""

# Call the D-Bus method
RESULT=$(busctl call com.linuxhello.FaceAuth /com/linuxhello/FaceAuth \
    com.linuxhello.FaceAuth StartCaptureStream uuu \
    $USER_ID $NUM_FRAMES $TIMEOUT_MS 2>&1) || {
    echo "❌ Error during the D-Bus call"
    echo "Result: $RESULT"
    exit 1
}

echo "✓ Result: $RESULT"
echo ""

echo "=========================================="
echo "✓ Test succeeded!"
echo "=========================================="
echo ""
echo "To observe the D-Bus signals, run:"
echo "  dbus-monitor --session"
echo ""
