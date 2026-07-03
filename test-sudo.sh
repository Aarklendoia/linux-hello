#!/bin/bash
# Test the Linux Hello PAM module with sudo

set -e

echo "=== Testing Linux Hello PAM with sudo ==="
echo ""

cd /home/edtech/Documents/linux-hello-rust

# Start the daemon
echo "1. Starting the daemon..."
./target/debug/hello-daemon --debug &
DAEMON_PID=$!
sleep 3
echo "   ✓ Daemon started (PID: $DAEMON_PID)"
echo ""

# Setup
USERNAME=$(whoami)
USER_ID=$(id -u)
echo "2. Test Setup:"
echo "   User: $USERNAME"
echo "   UID: $USER_ID"
echo ""

# Check that a face is registered
echo "3. Verification: registered faces..."
FACES=$(dbus-send --session --print-reply --dest=com.linuxhello.FaceAuth /com/linuxhello/FaceAuth com.linuxhello.FaceAuth.ListFaces uint32:"$USER_ID" 2>&1 | grep -o "face_id" | wc -l)
echo "   Faces found: $FACES"

if [ "$FACES" -eq 0 ]; then
    echo ""
    echo "   ⚠️  No face registered! Registering..."
    dbus-send --session --print-reply \
      --dest=com.linuxhello.FaceAuth \
      /com/linuxhello/FaceAuth \
      com.linuxhello.FaceAuth.RegisterFace \
      string:"{\"user_id\":$USER_ID,\"context\":\"sudo\",\"timeout_ms\":5000,\"num_samples\":1}" > /dev/null 2>&1
    echo "   ✓ Face registered"
fi
echo ""

# Test 1: Direct Verify call via D-Bus
echo "4. Test 1: Verification via direct D-Bus..."
VERIFY_REQUEST="{\"user_id\":$USER_ID,\"context\":\"sudo\",\"timeout_ms\":3000}"
if dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.Verify \
  string:"$VERIFY_REQUEST" 2>&1 | grep -q "Success"; then
    echo "   ✓ Verification succeeded via D-Bus"
else
    echo "   ✗ Verification failed"
fi
echo ""

# Test 2: Test with sudo (requires PAM configuration)
echo "5. Test 2: Attempt with sudo..."
echo "   Note: This depends on the system PAM configuration"
echo ""
echo "   To test with the local PAM configuration:"
echo "   sudo -p 'Password: ' -v"
echo ""
echo "   Or copy the PAM config:"
echo "   sudo cp sudo-linux-hello.pam /etc/pam.d/sudo"
echo "   sudo cp sudo-linux-hello.pam /etc/pam.d/sudo-i"
echo ""

# Test with direct PAM if possible
if command -v pamtester &> /dev/null; then
    echo "6. Test 3: With pamtester (if config available)..."
    # Create a test PAM config
    if [ -f /etc/pam.d/linux-hello-test ]; then
        echo "   Using config /etc/pam.d/linux-hello-test"
        # Note: pamtester reads from stdin, we pass it an empty password
        echo "" | pamtester -v linux-hello-test "$USERNAME" authenticate 2>&1 | head -5 || true
    else
        echo "   Config /etc/pam.d/linux-hello-test not found (ok for basic test)"
    fi
    echo ""
fi

# Stop the daemon
echo "7. Stopping the daemon..."
kill $DAEMON_PID 2>/dev/null || true
wait $DAEMON_PID 2>/dev/null || true
echo "   ✓ Daemon stopped"
echo ""

echo "=== Test complete ==="
echo ""
echo "Next steps:"
echo "1. Build in release mode: cargo build --release"
echo "2. Install: sudo install -m 644 target/release/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/"
echo "3. Configure PAM: sudo cp sudo-linux-hello.pam /etc/pam.d/sudo"
echo "4. Test: sudo -v (first facial authentication!)"
