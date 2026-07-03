#!/bin/bash
# Demo script: live video capture and preview display

set -e

echo "🎬 Demo: Linux Hello live video preview"
echo ""

# Start the daemon if needed
if ! systemctl --user is-active --quiet hello-daemon; then
    echo "▶️  Starting the daemon..."
    systemctl --user start hello-daemon
    sleep 2
fi

echo "📸 Calling StartCaptureStream via D-Bus..."
echo "   user_id=1000, num_frames=100, timeout=15000ms"
echo ""

# Start video capture
# Parameters: user_id (uint32)=1000, num_frames (uint32)=100, timeout_ms (uint64)=15000
RESULT=$(dbus-send --print-reply \
    --session \
    --dest=com.linuxhello.FaceAuth \
    /com/linuxhello/FaceAuth \
    com.linuxhello.FaceAuth.StartCaptureStream \
    uint32:1000 \
    uint32:100 \
    uint64:15000 2>&1) || {
    echo "⚠️  D-Bus call failed:"
    echo "$RESULT" | head -5
    echo ""
}

echo "$RESULT" | grep -E "string|OK" && echo "✅ Call launched successfully" || echo "⚠️  No confirmation"

echo ""
echo "⏳ Waiting for a few frames... (5 seconds)"
sleep 5

# Check that the preview file was created
if [ -f /tmp/linux-hello-preview.jpg ]; then
    SIZE=$(du -h /tmp/linux-hello-preview.jpg | cut -f1)
    echo "✅ Preview file created: /tmp/linux-hello-preview.jpg ($SIZE)"
    echo ""

    # Show image details
    echo "📊 Image details:"
    file /tmp/linux-hello-preview.jpg || true
    identify /tmp/linux-hello-preview.jpg 2>/dev/null || echo "   (ImageMagick not installed)"
else
    echo "❌ Preview file not found"
fi

echo ""
echo "▶️  Stopping the capture..."
dbus-send --print-reply \
    --session \
    --dest=com.linuxhello.FaceAuth \
    /com/linuxhello/FaceAuth \
    com.linuxhello.FaceAuth.StopCaptureStream 2>/dev/null || true

echo "✅ Demo complete"
echo ""
echo "💡 You can now launch the GUI:"
echo "   linux-hello-config"
echo ""
