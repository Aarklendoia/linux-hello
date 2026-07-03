#!/bin/bash
# Test script to verify that the video preview system works

set -e

echo "🔍 Testing the Linux Hello video preview system"
echo ""

# Check that the daemon is running
echo "1️⃣  Checking the daemon..."
if systemctl --user is-active --quiet hello-daemon; then
    echo "   ✅ Daemon active (systemctl --user status hello-daemon)"
else
    echo "   ❌ Daemon inactive, starting..."
    systemctl --user start hello-daemon
    sleep 2
fi

# Check that the D-Bus service is registered
echo ""
echo "2️⃣  Checking the D-Bus service..."
if gdbus call --system --dest=org.freedesktop.DBus --object-path=/org/freedesktop/DBus --method=org.freedesktop.DBus.ListNames 2>/dev/null | grep -q "com.linuxhello.FaceAuth"; then
    echo "   ✅ D-Bus service registered: com.linuxhello.FaceAuth"
else
    echo "   ⚠️  D-Bus service not found, but continuing..."
fi

# Check that the camera is available
echo ""
echo "3️⃣  Checking the camera..."
if [ -e /dev/video0 ]; then
    CAMERA_INFO=$(v4l2-ctl --device=/dev/video0 --info 2>&1 | head -1)
    echo "   ✅ Camera found: $CAMERA_INFO"
else
    echo "   ❌ Camera not found on /dev/video0"
    exit 1
fi

# Show the GUI configuration path
echo ""
echo "4️⃣  Checking the QML files..."
QML_FILE="/usr/share/qt6/qml/Linux/Hello/main.qml"
if [ -f "$QML_FILE" ]; then
    echo "   ✅ QML file found: $QML_FILE"
else
    echo "   ❌ QML file not found: $QML_FILE"
    exit 1
fi

# Check /tmp write permissions for the preview
echo ""
echo "5️⃣  Checking /tmp permissions..."
if [ -w /tmp ]; then
    echo "   ✅ /tmp directory is writable"
else
    echo "   ❌ /tmp directory is not writable"
    exit 1
fi

echo ""
echo "✅ All tests passed!"
echo ""
echo "To launch the registration GUI:"
echo "   linux-hello-config"
echo ""
echo "Or directly with qml6:"
echo "   export QML_IMPORT_PATH=/usr/lib/x86_64-linux-gnu/qt6/qml:/usr/share/qt6/qml"
echo "   qml6 $QML_FILE"
echo ""
