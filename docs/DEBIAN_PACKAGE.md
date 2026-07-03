# Debian Package Build & Installation

## Package Information

The Linux Hello project builds 4 Debian packages:

| Package | Size | Contents |
|---------|------|----------|
| `linux-hello` | 6.2K | Meta-package with documentation and scripts |
| `linux-hello-daemon` | 2.8K | Face authentication daemon + systemd service |
| `libpam-linux-hello` | 2.8K | PAM module for system integration |
| `linux-hello-tools` | 2.7K | CLI tools for face management |

## Building the Debian Package

### Prerequisites
```bash
# Install build dependencies
sudo apt-get update
sudo apt-get install -y \
  build-essential \
  debhelper \
  rustc \
  cargo \
  libdbus-1-dev \
  libpam0g-dev
```

### Build Command
```bash
cd /path/to/linux-hello-rust
dpkg-buildpackage -b -d
```

This will create 4 `.deb` files in the parent directory:
```
../linux-hello_1.0.0-1_amd64.deb
../linux-hello-daemon_1.0.0-1_amd64.deb
../libpam-linux-hello_1.0.0-1_amd64.deb
../linux-hello-tools_1.0.0-1_amd64.deb
```

## Installation

### Install All Components
```bash
sudo apt install /path/to/linux-hello_1.0.0-1_amd64.deb
```

This will:
- Install the daemon binary
- Install the PAM module
- Install CLI tools
- Launch interactive configuration wizard
- Create systemd user service
- Offer to configure sudo/screenlock PAM

### Install Specific Packages

**Daemon only:**
```bash
sudo dpkg -i linux-hello-daemon_1.0.0-1_amd64.deb
```

**PAM module only:**
```bash
sudo dpkg -i libpam-linux-hello_1.0.0-1_amd64.deb
```

**Tools only:**
```bash
sudo dpkg -i linux-hello-tools_1.0.0-1_amd64.deb
```

## Post-Installation Steps

### 1. Start the Daemon
```bash
systemctl --user enable hello-daemon.service
systemctl --user start hello-daemon.service
```

Check status:
```bash
systemctl --user status hello-daemon
journalctl --user -u hello-daemon -f
```

### 2. Register Your Face
```bash
linux-hello register --uid $(id -u) --device /dev/video0
```

### 3. Test Authentication

Test sudo:
```bash
sudo -l   # Should authenticate with your face
```

Test screenlock (KDE):
```bash
# Lock screen: Ctrl+Alt+L
# Present your face to camera for unlock
```

## Uninstallation

```bash
sudo apt remove linux-hello
```

The uninstaller will:
- Stop the daemon
- Remove PAM module
- Offer to restore PAM file backups

## File Locations

After installation, files are located at:

```
/usr/bin/
  ├── hello-daemon          # Main daemon binary
  └── linux-hello           # CLI tool

/lib/x86_64-linux-gnu/security/
  └── pam_linux_hello.so    # PAM module

/usr/lib/systemd/user/
  └── hello-daemon.service  # Systemd service

/etc/linux-hello/
  └── config.toml.example   # Example configuration

/usr/share/doc/linux-hello/
  ├── README.md
  ├── QUICKSTART.md
  ├── INTEGRATION_GUIDE.md
  ├── PAM_MODULE.md
  ├── sudo-linux-hello.pam
  └── kde-screenlock-linux-hello.pam
```

## Configuration Files Modified

The package automatically backs up and modifies:

- `/etc/pam.d/sudo` (if selected)
- `/etc/pam.d/kde` (if selected)

Backups are created with timestamps:
```
/etc/pam.d/sudo.pre-linuxhello-TIMESTAMP
/etc/pam.d/kde.pre-linuxhello-TIMESTAMP
```

## Troubleshooting

### Build Issues

**"debhelper not found"**
```bash
sudo apt install debhelper
```

**"rustc not found"**
```bash
sudo apt install rustc cargo
```

**"libdbus-1-dev not found"**
```bash
sudo apt install libdbus-1-dev
```

### Installation Issues

**"Cannot start daemon"**
```bash
systemctl --user restart hello-daemon
journalctl --user -u hello-daemon -n 50
```

**"PAM module not found"**
```bash
ldconfig -p | grep pam_linux_hello
ls -la /lib/x86_64-linux-gnu/security/pam_linux_hello.so
```

**"Camera not detected"**
```bash
ls -la /dev/video*
sudo usermod -a -G video $USER
```

### Restore from Backup

If PAM configuration is broken:

```bash
# List backups
ls -la /etc/pam.d/*.pre-linuxhello-*

# Restore the most recent
sudo cp /etc/pam.d/sudo.pre-linuxhello-LATEST /etc/pam.d/sudo

# Or run uninstaller
sudo apt remove linux-hello  # Choose "y" to restore
```

## Version Information

- **Package Version:** 1.0.0-1
- **Upstream Version:** 1.0.0
- **Release Type:** stable
- **Architecture:** amd64

## Debian Compatibility

Tested on:
- Debian 12 (Bookworm)
- Ubuntu 24.04 LTS

Should work on:
- Any Debian 11+ or Ubuntu 22.04+ system
- Both GNOME and KDE desktop environments
- systemd-based systems

## Development

### Rebuild After Code Changes

```bash
cd /path/to/linux-hello-rust
cargo build --release
dpkg-buildpackage -b -d --no-check-builddeps
```

### Update Version Number

Edit `debian/control` and `debian/changelog`:

```bash
# debian/control (find the Version field in Source section)
Version: 1.0.1-1

# debian/changelog (add new entry at top)
linux-hello (1.0.1-1) unstable; urgency=medium
  * Bug fixes and improvements
 -- Your Name <email>  DATE
```

## Package Dependencies

The .deb package declares the following dependencies:

- `dbus` (for D-Bus communication)
- `systemd` (for service management)
- `libc6` (standard C library)

Build dependencies (only needed when building):
- `rustc` (Rust compiler)
- `cargo` (Rust package manager)
- `libdbus-1-dev` (D-Bus development headers)
- `libpam0g-dev` (PAM development headers)

## Notes

1. **User Service:** The daemon runs as a systemd user service, not system-wide
2. **Backups:** Pre-installation backups are created automatically
3. **Password Fallback:** All authentication can fall back to password entry
4. **No Root Required:** Face registration and verification don't require sudo
5. **Security:** PAM files are properly backed up and restored

---

For more information, see:
- `/usr/share/doc/linux-hello/README.md` - Project overview
- `/usr/share/doc/linux-hello/QUICKSTART.md` - Getting started
- `/usr/share/doc/linux-hello/INTEGRATION_GUIDE.md` - Detailed integration steps
- `/usr/share/doc/linux-hello/PAM_MODULE.md` - PAM configuration reference
