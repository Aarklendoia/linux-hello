#!/bin/bash
# Test the Linux Hello PAM module

set -e

echo "=== Testing the Linux Hello PAM Module ==="
echo ""

# Start the daemon
echo "1. Starting the daemon..."
cd /home/edtech/Documents/linux-hello-rust
./target/debug/hello-daemon --debug &
DAEMON_PID=$!
sleep 3
echo "   Daemon PID: $DAEMON_PID"
echo ""

# Test with pamtester
echo "2. PAM test with the current user..."
USERNAME=$(whoami)
echo "   User: $USERNAME"
echo ""

echo "3. Calling pamtester..."
# pamtester asks for the password; we pass an empty one or use --version
# just to see whether the module can be loaded
pamtester -v linux-hello-test "$USERNAME" authenticate || {
    RESULT=$?
    if [ $RESULT -eq 0 ]; then
        echo "   ✓ Authentication succeeded (PAM_SUCCESS)"
    else
        echo "   ! Authentication failed or error: $RESULT"
    fi
}

echo ""
echo "4. Stopping the daemon..."
kill $DAEMON_PID 2>/dev/null || true
wait $DAEMON_PID 2>/dev/null || true
echo "   ✓ Daemon stopped"
echo ""

echo "=== Test complete ==="
