# 🚀 Quick Start - Linux Hello

## 5 Minutes to Test

### 1. Build (1 min)

```bash
cd ~/Documents/linux-hello-rust
cargo build --release
```

### 2. Enroll a Face (1 min)

```bash
./prepare-pam-test.sh
```

### 3. Test Sudo (1 min)

```bash
./test-sudo.sh
```

### 4. Test Screenlock (1 min)

```bash
./test-screenlock.sh
```

### 5. Check the Status (1 min)

```bash
./overview.sh
```

---

## Real Installation (10 minutes)

### Prerequisites
- Sudo rights
- Terminal
- Enrolled face (step 2 above)

### Steps

```bash
# 1. Install the PAM module
sudo install -m 644 target/release/libpam_linux_hello.so \
  /lib/x86_64-linux-gnu/security/pam_linux_hello.so

# 2. Backup sudo configuration
sudo cp /etc/pam.d/sudo /etc/pam.d/sudo.backup

# 3. Edit /etc/pam.d/sudo
sudo nano /etc/pam.d/sudo
```

In the editor, **add AT THE BEGINNING** (before any `auth`):

```
# Linux Hello - Face authentication for sudo
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sudo timeout_ms=3000 debug
```

Save: `Ctrl+O`, `Enter`, `Ctrl+X`

```bash
# 4. Start the daemon
./target/release/hello-daemon &

# 5. Test it!
sudo -v
```

You should be prompted for facial recognition!

---

## Problem? Restore!

```bash
# Restore original sudo
sudo cp /etc/pam.d/sudo.backup /etc/pam.d/sudo

# Stop daemon
pkill hello-daemon
```

---

## Useful Commands

```bash
# Start daemon with debug
./target/release/hello-daemon --debug

# List enrolled faces
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.ListFaces \
  uint32:$(id -u)

# Ping daemon
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.Ping

# View daemon logs
journalctl --user -u hello-daemon -f
```

---

## Full Docs

- `INTEGRATION_GUIDE.md` - Detailed installation + troubleshooting
- `PAM_MODULE.md` - Technical reference

---

**Happy testing! 🎉**
