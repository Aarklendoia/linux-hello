#!/bin/bash
# Linux Hello project overview

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║          Linux Hello - Linux Facial Authentication              ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""

cd /home/edtech/Documents/linux-hello-rust || exit 1

# Show project structure
echo "📁 Project Structure:"
echo ""
find . -maxdepth 2 -type d -not -path '*/target/*' -not -path '*/.git/*' | head -20 | sed 's/^\.\//   /'
echo ""

# Build
echo "🔨 Building..."
cargo build --release 2>&1 | grep -E "Compiling|Finished"
echo ""

# Show compiled artifacts
echo "📦 Artifacts:"
find target/release -maxdepth 1 \( -name "hello-daemon" -o -name "linux-hello" -o -name "libpam_linux_hello.so*" \) -type f 2>/dev/null | while read -r file; do
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    size_h=$(numfmt --to=iec-i --suffix=B "$size" 2>/dev/null || printf "%s\n" "$size")
    printf "   %s (%s)\n" "$file" "$size_h"
done
echo ""

# Show default config
echo "⚙️  Default Configuration:"
echo "   Storage: ~/.local/share/linux-hello"
echo "   D-Bus service: com.linuxhello.FaceAuth"
echo "   Similarity threshold: 0.6"
echo ""

# Show available tests
echo "✅ Available Tests:"
echo ""
echo "   Daemon tests:"
echo "      ./test-pam-full.sh    - Full daemon+D-Bus test"
echo ""
echo "   PAM integration tests:"
echo "      ./test-sudo.sh        - Test with sudo"
echo ""
echo "   Screenlock unlocking needs no PAM test: it's hello-daemon's own"
echo "   watcher (loginctl unlock-session on a face match) - just lock your"
echo "   screen while the daemon is running."
echo ""
echo "   Preparation:"
echo "      ./prepare-pam-test.sh - Register a face"
echo ""

# Show useful commands
echo "🚀 Useful Commands:"
echo ""
echo "   # Start the daemon"
echo "   ./target/release/hello-daemon --debug"
echo ""
echo "   # Register a face"
echo "   dbus-send --session --print-reply \\"
echo "     --dest=com.linuxhello.FaceAuth \\"
echo "     /com/linuxhello/FaceAuth \\"
echo "     com.linuxhello.FaceAuth.RegisterFace \\"
echo "     string:'{\"user_id\":1000,\"context\":\"test\",\"timeout_ms\":5000,\"num_samples\":1}'"
echo ""
echo "   # Verify a face"
echo "   dbus-send --session --print-reply \\"
echo "     --dest=com.linuxhello.FaceAuth \\"
echo "     /com/linuxhello/FaceAuth \\"
echo "     com.linuxhello.FaceAuth.Verify \\"
echo "     string:'{\"user_id\":1000,\"context\":\"test\",\"timeout_ms\":3000}'"
echo ""

# Show documentation
echo "📚 Documentation:"
echo ""
echo "   README.md                   - Overview"
echo "   docs/PAM_MODULE.md          - PAM module documentation"
echo "   docs/INTEGRATION_GUIDE.md   - sudo integration guide"
echo ""

# Show configuration files
echo "⚙️  PAM Configurations:"
echo ""
for f in sudo-linux-hello.pam test-pam-config; do
    if [ -f "$f" ]; then
        echo "   ✓ $f"
    else
        echo "   ✗ $f"
    fi
done
echo ""

# Show daemon status
echo "📡 Runtime Status:"
if dbus-send --session --print-reply --dest=com.linuxhello.FaceAuth /com/linuxhello/FaceAuth com.linuxhello.FaceAuth.Ping 2>/dev/null | grep -q "pong"; then
    echo "   ✓ D-Bus Daemon: Active"
else
    echo "   ✗ D-Bus Daemon: Inactive (run: ./target/release/hello-daemon)"
fi
echo ""

# Show next steps
echo "📋 Next Steps:"
echo ""
echo "   1. Test the daemon:"
echo "      ./target/release/hello-daemon &"
echo "      ./prepare-pam-test.sh"
echo ""
echo "   2. Test PAM with sudo:"
echo "      ./test-sudo.sh"
echo ""
echo "   3. System installation:"
echo "      sudo install -m 644 target/release/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/"
echo "      sudo nano /etc/pam.d/sudo  # add the linux-hello lines"
echo ""
echo "   4. Configure daemon on startup:"
echo "      mkdir -p ~/.config/systemd/user"
echo "      # See docs/INTEGRATION_GUIDE.md for details"
echo ""
echo "   5. Implement real camera:"
echo "      See hello_camera/src/lib.rs"
echo ""

echo "═══════════════════════════════════════════════════════════════════"
echo ""
