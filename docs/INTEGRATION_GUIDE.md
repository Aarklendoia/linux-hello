# Linux Hello Integration Guide - PAM Sudo

## Overview

This guide explains how to integrate Linux Hello into your system for
**sudo** - facial authentication for privilege escalation.

Screen unlocking via facial recognition is a separate feature that needs no
PAM configuration — see [Step 6](#step-6-screenlock-no-pam-configuration-needed) below.

> **If you installed via the `.deb` packages**, sudo activates automatically
> once you enroll a face — no manual PAM editing needed. Screenlock unlocking
> needs no PAM configuration at all (see
> [PAM_MODULE.md](PAM_MODULE.md#automatic-activation) for how both work).
> The manual sudo steps below remain useful for development (building from
> source) or for configuring services the automatic path doesn't touch
> (e.g. `sddm`).

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

## Step 6: Screenlock (no PAM configuration needed)

Unlike sudo, screen unlocking doesn't go through PAM at all: `hello-daemon`
polls `org.freedesktop.ScreenSaver` over D-Bus while it's running, and on a
face match while the screen is locked, unlocks the session directly via
`loginctl unlock-session` (see `hello_daemon/src/screenlock.rs`). As long as
the daemon is running and you've enrolled a face, locking your screen and
looking at the camera is enough — nothing to edit under `/etc/pam.d/`.

An earlier revision of this project tried wiring this up as a PAM line
(`context=screenlock`) inserted into a KDE-specific service file
(`kde-screenlocker`/`kde`), but current KDE Plasma (6.x) doesn't ship or use
either by default, so that approach never actually ran in practice — the
watcher above replaced it entirely.

### Screenlock Test

```bash
# Make sure the daemon is running and a face is enrolled, then lock your
# screen (e.g. Meta+L) and look at the camera.
systemctl --user status hello-daemon.service
journalctl --user -u hello-daemon.service -f
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

**If you installed via the `.deb` packages**, this is already done:
`linux-hello`'s postinst enables and starts the packaged
`/usr/lib/systemd/user/hello-daemon.service` for you
(`systemctl --user enable --now hello-daemon.service`). Nothing further
is needed.

**If you're running from a source build** (no package installed), install
and enable the unit shipped at the repo root yourself:

```bash
mkdir -p ~/.config/systemd/user
cp hello-daemon.service ~/.config/systemd/user/
# Point ExecStart at your build instead of /usr/bin/hello-daemon:
sed -i "s#/usr/bin/hello-daemon#$(pwd)/target/release/hello-daemon#" \
  ~/.config/systemd/user/hello-daemon.service

systemctl --user daemon-reload
systemctl --user enable --now hello-daemon.service
systemctl --user status hello-daemon.service
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
