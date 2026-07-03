# Detailed Design: D-Bus and PAM

## 1. D-Bus Interface

### Service

- **Name**: `com.linuxhello.FaceAuth`
- **Path**: `/com/linuxhello/FaceAuth`
- **Interface**: `com.linuxhello.FaceAuth`

### Methods

#### `RegisterFace(s: request) -> s: response`

Enroll a new face for a user.

**Input (JSON)**:

```json
{
  "user_id": 1000,
  "context": "login",
  "timeout_ms": 5000,
  "num_samples": 3
}
```

**Output (JSON)**:

```json
{
  "face_id": "face_20250106_1410_1",
  "registered_at": 1735036800,
  "quality_score": 0.95
}
```

**Errors**:

- `com.linuxhello.AccessDenied`: User doesn't have permission
- `com.linuxhello.CameraError`: Camera unavailable
- `com.linuxhello.StorageError`: Storage error

---

#### `DeleteFace(s: request) -> ()`

Delete one or all faces.

**Input (JSON)**:

```json
{
  "user_id": 1000,
  "face_id": "face_20250106_1410_1"
}
```

Or (all faces):

```json
{
  "user_id": 1000,
  "face_id": null
}
```

---

#### `Verify(s: request) -> s: result`

Verify a user's identity.

**Input (JSON)**:

```json
{
  "user_id": 1000,
  "context": "login",
  "timeout_ms": 5000
}
```

**Output (JSON - Success)**:

```json
{
  "type": "Success",
  "face_id": "face_20250106_1410_1",
  "similarity_score": 0.87
}
```

**Output (JSON - Failure)**:

```json
{
  "type": "NoMatch",
  "best_score": 0.45,
  "threshold": 0.60
}
```

**Other types**:

- `NoFaceDetected`: No face in the camera
- `NoEnrollment`: No face enrolled for this UID
- `Cancelled`: User cancelled (black screen, timeout, etc.)
- `Error`: Internal error message

---

#### `ListFaces(u: user_id) -> s: faces_json`

List all enrolled faces.

**Output (JSON)**:

```json
[
  {
    "face_id": "face_20250106_1410_1",
    "registered_at": 1735036800,
    "quality_score": 0.95,
    "context": "login"
  },
  {
    "face_id": "face_20250106_1415_1",
    "registered_at": 1735037100,
    "quality_score": 0.92,
    "context": "sudo"
  }
]
```

---

### Properties

#### `Version: s` (read-only)

Daemon version (e.g. "0.1.0")

#### `CameraAvailable: b` (read-only)

Boolean indicating whether a camera is available

---

## 2. PAM Configuration

### General syntax

```text
auth   [module_path] [service_name] [module_name] [arguments...]
```

### Module options

| Option | Value | Default | Description |
| ------ | ----- | ------- | ----------- |
| `context` | string | "default" | Authentication context |
| `timeout_ms` | u64 | 5000 | Max timeout in ms |
| `similarity_threshold` | f32 | 0.6 | Similarity threshold (0.0-1.0) |
| `confirm` | (flag) | false | Ask for confirmation before success |
| `debug` | (flag) | false | Debug mode |

### Per-service configurations

#### `/etc/pam.d/login` (TTY)

```text
auth   sufficient   pam_linux_hello.so context=login timeout_ms=5000
auth   include      system-login
```

#### `/etc/pam.d/sudo`

```text
auth   sufficient   pam_linux_hello.so context=sudo confirm=true
auth   include      system-auth
```

#### `/etc/pam.d/kde` (KScreenLocker)

```text
auth   sufficient   pam_linux_hello.so context=screenlock timeout_ms=3000
auth   include      system-login
```

#### `/etc/pam.d/sddm` (SDDM login)

```text
auth   sufficient   pam_linux_hello.so context=sddm timeout_ms=5000
auth   include      system-login
```

---

## 3. Authentication Flow

### Full flow: sudo

```text
User: $ sudo ls
    ↓
PAM (sudo)
    ↓
pam_linux_hello.so:pam_sm_authenticate()
    ├─ Parses PAM options (context=sudo, confirm=true, etc.)
    ├─ Retrieves PAM_USER and PAM_RHOST
    ├─ Calls D-Bus: Verify {user_id, context="sudo", timeout_ms=5000}
    │   ↓
    │   Face daemon
    │   ├─ Opens camera
    │   ├─ Captures frame, detects face
    │   ├─ Extracts embedding
    │   ├─ Compares with stored embeddings
    │   └─ Returns MatchResult
    │
    ├─ If Success and confirm=true:
    │   ├─ pam_conv() → displays "Confirm sudo? [y/N]"
    │   ├─ Waits for user response
    │   └─ If "y" → PAM_SUCCESS, otherwise → PAM_AUTH_ERR
    │
    ├─ If Success and confirm=false:
    │   └─ PAM_SUCCESS
    │
    └─ If failure:
       ├─ If NoFaceDetected → PAM_IGNORE (let password continue)
       └─ If NoMatch → PAM_IGNORE (same)
    ↓
PAM Return
    ├─ PAM_SUCCESS → sudo accepted
    ├─ PAM_IGNORE → continues with password
    └─ PAM_AUTH_ERR → sudo denied
```

---

### Flow: KScreenLocker

```text
User: Time to unlock the screen
    ↓
KScreenLocker unlocks
    ├─ Invokes the "kde" PAM service
    ├─ pam_sm_authenticate() → Verify {context="screenlock"}
    │
    ├─ If Success:
    │   └─ PAM_SUCCESS → screen unlocked
    │
    └─ If failure:
       ├─ PAM_IGNORE → displays password field
       └─ User enters password
```

---

## 4. Serialization and Protocol

### Choice: JSON over D-Bus

**Advantages:**

- Simple, human-readable
- Extensible (new fields without breaking)
- Easy to log/audit

**Disadvantages:**

- Less compact than CBOR/protobuf
- Slightly more expensive parsing

**Acceptable trade-off** for auth (low frequency, security >> performance)

### Example: PAM module calls the daemon

PAM → D-Bus:

```rust
let request = VerifyRequest {
    user_id: 1000,
    context: "sudo".to_string(),
    timeout_ms: 5000,
};
let json = serde_json::to_string(&request)?;

// Call D-Bus
let response_json = dbus_proxy.call("Verify", &json).await?;

let result: VerifyResult = serde_json::from_str(&response_json)?;
```

---

## 5. Error Handling

### Error levels

1. **User-facing** (via PAM_TEXT_INFO/ERROR):
   - "Recognition failed, try the password"
   - "Camera unavailable"
   - "Capture timeout"

2. **Admin logs** (via tracing):
   - Technical details
   - Timestamps
   - UID, context, scores

3. **PAM return codes**:
   - `PAM_SUCCESS`: OK
   - `PAM_AUTH_ERR`: Auth failure
   - `PAM_IGNORE`: Ignore this module
   - `PAM_SYSTEM_ERR`: System error

---

## 6. D-Bus Security

### PolicyKit rules (optional for root daemon)

Create `/usr/share/polkit-1/rules.d/com.linuxhello.rules`:

```javascript
polkit.addRule(function(action, subject) {
    if (action.id == "com.linuxhello.RegisterFace") {
        // User can enroll their own face
        if (subject.user == action.lookup("user")) {
            return polkit.Result.YES;
        }
    }
    if (action.id == "com.linuxhello.Verify") {
        // User can verify their own face
        if (subject.user == action.lookup("user")) {
            return polkit.Result.YES;
        }
    }
    return polkit.Result.NOT_HANDLED;
});
```

### Simple ACL (without Polkit)

In the daemon:

```rust
fn check_permission(current_uid: u32, target_uid: u32) -> Result<()> {
    // Root = always OK
    if current_uid == 0 { return Ok(()); }
    
    // A user can only modify their own face
    if current_uid != target_uid {
        return Err(AccessDenied);
    }
    Ok(())
}
```

---

## 7. Logging and Auditing

### Standardized format

```text
[2025-01-06T14:10:23Z] [INFO] pam_linux_hello: user=alice uid=1000 context=sudo result=success score=0.87
[2025-01-06T14:10:24Z] [INFO] hello_daemon: RegisterFace uid=1000 face_id=face_1410_1 quality=0.95
[2025-01-06T14:10:25Z] [ERROR] hello_camera: V4L2 open failed: /dev/video0 not found
```

### Destinations

- Stderr if daemon is interactive
- `/var/log/linux-hello.log` if systemd service
- Systemd journal if available

---

## 8. Tests

### Local D-Bus daemon test

```bash
# Terminal 1: start the daemon
cargo run -p linux_hello_cli -- daemon --debug

# Terminal 2: call the daemon
dbus-send --session \
  --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.Ping
```

### PAM Test

```bash
# Custom test service
sudo nano /etc/pam.d/linux-hello-test

# Content:
# auth sufficient pam_linux_hello.so debug context=test
# account required pam_permit.so
# session required pam_permit.so

# Test
pamtester linux-hello-test $USER authenticate

# Or
su -s /bin/sh -c "echo It works" - $USER
```
