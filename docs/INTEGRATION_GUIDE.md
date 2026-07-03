# Linux Hello Integration Guide - PAM Sudo & Screenlock

## Overview

This guide explains how to integrate Linux Hello into your system for:

1. **sudo** - Facial authentication for privilege escalation
2. **Screenlock** - Screen unlocking via facial recognition

## Prerequisites

- [ ] Compiled PAM module: `libpam_linux_hello.so`
- [ ] Linux Hello daemon: `hello-daemon`
- [ ] Faces enrolled for your user
- [ ] D-Bus session running

## Step 1: Release Build

```bash
cd ~/Documents/linux-hello-rust

# Build in release mode (optimized)
cargo build --release

# Check the .so
ls -lh target/release/libpam_linux_hello.so
```

## Step 2: PAM Module Installation

**IMPORTANT**: This requires root privileges. Be careful!

```bash
# Install the module
sudo install -m 644 target/release/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/pam_linux_hello.so

# Verify
ls -l /lib/x86_64-linux-gnu/security/pam_linux_hello.so
```

## Step 3: Sudo Configuration

### Option A: Use existing configuration (RECOMMENDED FOR TESTING)

```bash
# Backup the original
sudo cp /etc/pam.d/sudo /etc/pam.d/sudo.backup

# Edit with sudo
sudo nano /etc/pam.d/sudo
```

Add **AT THE BEGINNING** of the file (before the other auth lines):

```
# Linux Hello - Facial authentication for sudo
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sudo timeout_ms=3000 debug
```

**Full example of /etc/pam.d/sudo:**

```
# /etc/pam.d/sudo: ~/.pam_environment is not read
#%PAM-1.0

# Linux Hello - Facial authentication
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sudo timeout_ms=3000 debug

# Defaults for environment variables on Debian systems
session required pam_permit.so

# Enable the below to restrict root login to only those interfaces that are also allowed for non-root login
# auth    required    pam_wheel.so
# or
# auth    required    pam_unix.so nullok try_first_pass yescrypt root_unlock_only
auth    required    pam_unix.so nullok try_first_pass yescrypt

# This includes support for password authentication, including PAM keyboard-
# interactive and PAM generic mechanisms (such as the experimental OPIE
# support)
session [optional=ignore success=ok ignore=ignore module_unknown=ignore default=bad] pam_umask.so umask=0022

session    required                        pam_unix.so
session    optional                        pam_lastlog.so showfailed
session    optional                        pam_motd.so  motd=/run/motd.dynamic
session    optional                        pam_mail.so standard
```

### Option B: Create a custom config

```bash
sudo cp sudo-linux-hello.pam /etc/pam.d/sudo-linux-hello
```

## Step 4: Enroll a Face for Sudo Authentication

Before testing, make sure a face is enrolled:

```bash
# Start the daemon
./target/debug/hello-daemon &

# Enroll a face
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.RegisterFace \
  string:'{"user_id":'$(id -u)',"context":"sudo","timeout_ms":5000,"num_samples":3}'

# Stop the daemon
pkill hello-daemon
```

## Step 5: Sudo Test

### Test 1: Check that the module is loaded

```bash
# Start the daemon
./target/debug/hello-daemon --debug &
sleep 2

# Test authentication
sudo -v
```

Wait for your camera to start (or simulate the capture). If the module is loaded, you should see:

- Daemon logs showing "D-Bus call: verify"
- Your terminal prompting you to authenticate

### Test 2: Run a command with sudo

```bash
# Start the daemon
./target/debug/hello-daemon &

# Run a command with sudo
sudo ls /root

# On success: the command runs
# On failure: sudo prompts for the password
```

### Test 3: Use the automated test script

```bash
./test-sudo.sh
```

## Step 6: KDE Screenlock Configuration

### Locate the screenlock ID

```bash
# KDE Plasma 5.20+
ls -la /etc/pam.d/ | grep kde

# Look for kde, kde-screenlocker, kdesu, etc.
```

### Configure the screenlock

**Option A: Modify the existing config**

```bash
# Backup the original
sudo cp /etc/pam.d/kde /etc/pam.d/kde.backup

# Edit
sudo nano /etc/pam.d/kde
```

Add AT THE BEGINNING:

```
# Linux Hello - Facial authentication for screenlock
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=screenlock timeout_ms=3000 debug
```

**Option B: Use the provided config**

```bash
sudo cp kde-screenlock-linux-hello.pam /etc/pam.d/kde
```

### Screenlock Test

```bash
# Start the daemon
./target/debug/hello-daemon &

# Run the test
./test-screenlock.sh

# Or test manually with the screensaver
# Press Ctrl+Alt+L or use the KDE menu
```

## Security: Important Points

### ⚠️ Password Fallback

If the PAM module fails or the daemon is unavailable, **you can always use your password**.

The `auth sufficient` configuration means:

- If linux-hello succeeds → full authentication
- If linux-hello fails → use the next method (pam_unix = password)

### 🔒 Backups

**ALWAYS make a backup before modifying PAM:**

```bash
# Backup all configs
sudo cp -r /etc/pam.d /etc/pam.d.backup.$(date +%Y%m%d-%H%M%S)

# In case of problems, restore:
# sudo cp /etc/pam.d/sudo.backup /etc/pam.d/sudo
```

### 🚨 Emergency Restoration

If you get locked out of the system:

1. **Boot in recovery/single-user mode**
2. **Restore the files**:

```bash
# Mount the filesystem read-write
mount -o rw,remount /

# Restore
cp /etc/pam.d.backup.YYYYMMDD-HHMMSS/sudo /etc/pam.d/sudo
cp /etc/pam.d.backup.YYYYMMDD-HHMMSS/kde /etc/pam.d/kde

# Reboot
reboot
```

## Troubleshooting

### Error: "pam_linux_hello.so not found"

```bash
# Check the location
ls -l /lib/x86_64-linux-gnu/security/pam_linux_hello.so

# If missing, reinstall
sudo install -m 644 target/release/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/
```

### Error: "Cannot connect to D-Bus"

```bash
# Check that the D-Bus session is running
echo $DBUS_SESSION_BUS_ADDRESS

# If empty, restart it
eval $(dbus-launch --sh-syntax)

# Restart the daemon
./target/debug/hello-daemon
```

### Error: "Name already taken on the bus"

```bash
# The daemon is already running
pkill hello-daemon

# Wait and restart
sleep 2
./target/debug/hello-daemon
```

### Error: "Unable to retrieve UID for user"

```bash
# Check that the user exists
id $USER
```

### sudo asks for the password instead of facial recognition

```bash
# Check the PAM config
cat /etc/pam.d/sudo | head -10

# Check that the module is installed
ls -l /lib/x86_64-linux-gnu/security/pam_linux_hello.so

# Check that the daemon is running
ps aux | grep hello-daemon

# Check that faces are enrolled
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.ListFaces \
  uint32:$(id -u)
```

## Automatic Daemon Startup

To have the daemon start automatically at boot:

### Option 1: systemd user service

```bash
mkdir -p ~/.config/systemd/user

cat > ~/.config/systemd/user/hello-daemon.service << 'EOF'
[Unit]
Description=Linux Hello Face Authentication Daemon
After=dbus.service

[Service]
Type=notify
ExecStart=/home/YOUR_USERNAME/Documents/linux-hello-rust/target/release/hello-daemon
Restart=on-failure

[Install]
WantedBy=default.target
EOF

# Enable
systemctl --user enable hello-daemon.service
systemctl --user start hello-daemon.service

# Verify
systemctl --user status hello-daemon.service
```

### Option 2: xinitrc/startuprc (desktop environment specific)

Add to `~/.xinitrc` or `~/.kde4/Autostart`:

```bash
~/Documents/linux-hello-rust/target/release/hello-daemon &
```

## Next Steps

- [ ] Build in release
- [ ] Install the module
- [ ] Test with sudo
- [ ] Test with screenlock
- [ ] Configure automatic daemon startup
- [ ] Document deployment for other users

## Support

For bugs or questions:

1. Check the logs: `journalctl --user -u hello-daemon`
2. Enable debug: `debug` option in PAM
3. See PAM_MODULE.md for advanced options

---

**Version**: 0.1.0
**Date**: January 2026
**Status**: Beta - Ready for personal testing
