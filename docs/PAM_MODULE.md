# Linux Hello PAM Module

PAM authentication module for Linux Hello - enables facial authentication via PAM for login, sudo, screenlock, etc.

## Building

```bash
cargo build -p pam_linux_hello --release
```

The compiled module will be at `target/release/libpam_linux_hello.so`

## Installation

### System installation

```bash
sudo install -m 644 target/release/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/
```

### Or local test installation

Use the full path in the PAM configuration to test without root privileges.

## PAM Configuration

### Basic format

```text
auth [sufficient|required] /path/to/libpam_linux_hello.so [options]
```

### Available options

- `context=<context>`: Authentication context (login, sudo, screenlock, sddm, test, etc.) [default: default]
- `timeout_ms=<ms>`: Timeout in milliseconds for capture [default: 5000]
- `similarity_threshold=<0.0-1.0>`: Similarity threshold [default: 0.6]
- `debug`: Enable debug logs

### Usage Examples

#### Login (SDDM/GDM)

```bash
# /etc/pam.d/sddm
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sddm timeout_ms=5000
auth include common-auth
```

#### Sudo

```bash
# /etc/pam.d/sudo
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sudo timeout_ms=3000
@include common-auth
```

#### Screenlock (KDE/GNOME)

```bash
# /etc/pam.d/kde or /etc/pam.d/gnome
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=screenlock timeout_ms=3000
auth required pam_permit.so
```

## PAM Return Codes

- `PAM_SUCCESS`: Authentication succeeded (face recognized)
- `PAM_AUTH_ERR`: Authentication failed (face not recognized or system error)
- `PAM_SYSTEM_ERR`: System error (daemon unavailable, etc.)
- `PAM_IGNORE`: Module cannot authenticate (debug mode)

## Recommended Contexts and Thresholds

Similarity thresholds vary depending on the context:

| Context | Default Threshold | Recommendation |
| ---------- | ------------------ | --- |
| login | 0.65 | Strict |
| sddm | 0.65 | Strict |
| sudo | 0.70 | Very strict |
| screenlock | 0.60 | Moderate |
| test | 0.50 | Permissive (test) |

## System Dependencies

The PAM module requires:

- D-Bus session bus running
- Linux Hello daemon running (`hello-daemon`)
- Faces enrolled for the user

## Testing

### Direct D-Bus test (without PAM)

```bash
# Start the daemon
./target/debug/hello-daemon --debug &

# Enroll a face
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.RegisterFace \
  string:'{"user_id":1000,"context":"test","timeout_ms":5000,"num_samples":3}'

# Verify the face
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.Verify \
  string:'{"user_id":1000,"context":"test","timeout_ms":5000}'
```

### PAM Test with pamtester

```bash
# See the project sources for the full test script
./test-pam-full.sh
```

## Security

The PAM module implements:

- Verification based on the user's UID
- Access to the D-Bus session daemon (session isolation)
- Structured logs for auditing
- Timeouts to prevent blocking

## Troubleshooting

### "The name com.linuxhello.FaceAuth was not provided by any .service files"

The Linux Hello daemon is not running. Start it:

```bash
./target/debug/hello-daemon
```

### "Unable to retrieve UID for user"

The user doesn't exist or `getpwnam` is unavailable. Check with:

```bash
id username
```

### Module doesn't compile

Make sure the Rust dependencies are up to date:

```bash
cargo update -p hello_daemon -p pam_linux_hello
```

## Architecture

```text
Login/Sudo/Screenlock
         |
         v
   PAM Stack
         |
         v
   pam_linux_hello.so
         |
         v
    D-Bus session
         |
         v
  hello-daemon
         |
         v
 Camera + Face Matching
```

## Current Limitations

- Uses simulated camera (virtual frames)
- No multi-face support per probe
- Global timeout for capture+matching
- No persistent logging

## Future Improvements

- [ ] Real camera integration (V4L2/PipeWire)
- [ ] Real machine learning (ONNX/TensorFlow)
- [ ] Multi-modal support (IR, Depth)
- [ ] Polkit for sudo without PAM
- [ ] REST API in addition to D-Bus
- [ ] Persistent database (sqlite)
