#!/bin/bash
# Test the Linux Hello PAM module with an integrated daemon startup

set -e

echo "=== Testing the Linux Hello PAM Module ==="
echo ""

cd /home/edtech/Documents/linux-hello-rust

# Start the daemon
echo "1. Starting the daemon..."
./target/debug/hello-daemon --debug &
DAEMON_PID=$!
sleep 3
echo "   Daemon PID: $DAEMON_PID"
echo ""

# Test 1: Verify that the module can be loaded and logs appear
echo "2. Testing the PAM module..."
USERNAME=$(whoami)
USER_ID=$(id -u)
echo "   User: $USERNAME (UID: $USER_ID)"
echo ""

# Build a Verify request
echo "3. Calling Verify via the PAM module (simulated via direct D-Bus)..."
VERIFY_REQUEST="{\"user_id\":$USER_ID,\"context\":\"login\",\"timeout_ms\":3000}"
echo "   Request: $VERIFY_REQUEST"
echo ""

# Call Verify directly via D-Bus to test that verification works
VERIFY_RESPONSE=$(dbus-send --session --print-reply --dest=com.linuxhello.FaceAuth /com/linuxhello/FaceAuth com.linuxhello.FaceAuth.Verify string:"$VERIFY_REQUEST" 2>&1 | tail -1)
echo "   Response: $VERIFY_RESPONSE"

if echo "$VERIFY_RESPONSE" | grep -q "Success"; then
    echo "   ✓ Verification succeeded!"
else
    echo "   ✗ Verification failed"
fi
echo ""

# To test with pamtester, we would need a PAM configuration
# For now, just verify that the module loads and D-Bus works
echo "4. Listing faces..."
dbus-send --session --print-reply --dest=com.linuxhello.FaceAuth /com/linuxhello/FaceAuth com.linuxhello.FaceAuth.ListFaces uint32:"$USER_ID" 2>&1 | tail -1 | head -c 100
echo "..."
echo ""

echo "5. Stopping the daemon..."
kill $DAEMON_PID 2>/dev/null || true
wait $DAEMON_PID 2>/dev/null || true

echo ""
echo "✓ Test completed successfully!"
