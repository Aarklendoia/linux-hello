#!/bin/bash
# Prepare the test: register a face for the current user

set -e

echo "=== Preparing the PAM test ==="
echo ""

cd /home/edtech/Documents/linux-hello-rust

# Start the daemon
echo "1. Starting the daemon..."
./target/debug/hello-daemon --debug &
DAEMON_PID=$!
sleep 3

echo "2. Registering a face for the current user..."
USER_ID=$(id -u)

# Call RegisterFace via D-Bus
REQUEST="{\"user_id\":$USER_ID,\"context\":\"test\",\"timeout_ms\":5000,\"num_samples\":1}"
echo "   Request: $REQUEST"
echo ""

RESPONSE=$(dbus-send --session --print-reply --dest=com.linuxhello.FaceAuth /com/linuxhello/FaceAuth com.linuxhello.FaceAuth.RegisterFace string:"$REQUEST" 2>&1 | tail -1)
echo "   Response: $RESPONSE"
echo ""

echo "3. Verification: enumerating faces..."
dbus-send --session --print-reply --dest=com.linuxhello.FaceAuth /com/linuxhello/FaceAuth com.linuxhello.FaceAuth.ListFaces uint32:"$USER_ID" 2>&1 | tail -1 | head -c 100
echo "..."
echo ""

echo "4. Stopping the daemon..."
kill $DAEMON_PID 2>/dev/null || true
wait $DAEMON_PID 2>/dev/null || true

echo ""
echo "✓ Preparation complete!"
echo "You can now run: ./test-pam.sh"
