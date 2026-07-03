#!/bin/bash
# Test the Linux Hello PAM module with screenlock
# Simulates the authentication flow of a screenlock

set -e

echo "=== Testing Linux Hello PAM with KDE Screenlock ==="
echo ""

cd /home/edtech/Documents/linux-hello-rust

# Start the daemon
echo "1. Starting the daemon..."
./target/debug/hello-daemon --debug &
DAEMON_PID=$!
sleep 3
echo "   ✓ Daemon started (PID: $DAEMON_PID)"
echo ""

USERNAME=$(whoami)
USER_ID=$(id -u)

echo "2. Configuration:"
echo "   User: $USERNAME (UID: $USER_ID)"
echo "   Context: screenlock"
echo "   Timeout: 3000ms"
echo ""

# Check/create faces
echo "3. Preparation: registered faces..."
FACES=$(dbus-send --session --print-reply --dest=com.linuxhello.FaceAuth /com/linuxhello/FaceAuth com.linuxhello.FaceAuth.ListFaces uint32:"$USER_ID" 2>&1 | grep -o "face_id" | wc -l)
echo "   Faces found: $FACES"

if [ "$FACES" -eq 0 ]; then
    echo "   → Registering a new face..."
    dbus-send --session --print-reply \
      --dest=com.linuxhello.FaceAuth \
      /com/linuxhello/FaceAuth \
      com.linuxhello.FaceAuth.RegisterFace \
      "string:{\"user_id\":$USER_ID,\"context\":\"screenlock\",\"timeout_ms\":5000,\"num_samples\":1}" > /dev/null 2>&1
    echo "   ✓ Face registered"
fi
echo ""

# Simulation of screenlock authentication
echo "4. Simulation: screenlock authentication flow..."
echo ""
echo "   Calling: dbus-send → Verify (context=screenlock)"
echo ""

VERIFY_REQUEST="{\"user_id\":$USER_ID,\"context\":\"screenlock\",\"timeout_ms\":3000}"
RESPONSE=$(dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.Verify \
  string:"$VERIFY_REQUEST" 2>&1)

echo "$RESPONSE" | tail -5
echo ""

# Check the response
if echo "$RESPONSE" | grep -q "Success"; then
    echo "   ✅ AUTHENTICATION SUCCEEDED!"
    echo "      → The screen would be unlocked"
    RESULT=0
else
    echo "   ❌ AUTHENTICATION FAILED"
    echo "      → The user would need to enter their password"
    RESULT=1
fi
echo ""

echo "5. Module installation:"
echo "   To enable on the system:"
echo ""
echo "   sudo install -m 644 target/debug/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/"
echo "   sudo cp kde-screenlock-linux-hello.pam /etc/pam.d/kde"
echo ""
echo "   Or for KDE Plasma 5.27+:"
echo "   sudo cp kde-screenlock-linux-hello.pam /etc/pam.d/kde-screenlocker"
echo ""

# Stop the daemon
echo "6. Stopping the daemon..."
kill $DAEMON_PID 2>/dev/null || true
wait $DAEMON_PID 2>/dev/null || true
echo "   ✓ Daemon stopped"
echo ""

if [ $RESULT -eq 0 ]; then
    echo "✅ Screenlock test succeeded!"
    exit 0
else
    echo "❌ Screenlock test failed"
    exit 1
fi
