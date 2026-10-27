# D-Bus API: `com.linuxhello.FaceAuth`

The per-user `hello-daemon` exposes face enrollment and verification on the
**session bus**. The CLI (`linux-hello`) and the settings app use this API, and
so can third-party tools.

| | |
| --- | --- |
| Bus | session (one daemon per logged-in user, `hello-daemon.service` user unit) |
| Well-known name | `com.linuxhello.FaceAuth` |
| Object path | `/com/linuxhello/FaceAuth` |
| Interface | `com.linuxhello.FaceAuth` |
| Introspection XML | [`dbus/com.linuxhello.FaceAuth.xml`](dbus/com.linuxhello.FaceAuth.xml) |

The XML file is generated from the daemon's own zbus interface, and a unit test
fails when the two differ. After changing the interface (or a method's doc
comment, which the XML includes), regenerate it and commit the result:

```fish
env LINUX_HELLO_UPDATE_DBUS_XML=1 cargo test -p hello_daemon dbus_introspection_xml
```

Use it with `gdbus-codegen`, `qdbusxml2cpp`, `zbus-xmlgen` and similar tools,
or simply browse the service with `busctl --user introspect com.linuxhello.FaceAuth /com/linuxhello/FaceAuth`.

Other parts of the system (PAM, the SDDM login screen) don't go through D-Bus:
they use Unix sockets under `/run/hello-pam/`, which are internal and not part
of this API.

## Access control

- Only processes of the same user can reach a session bus, so every call comes
  from the user the daemon runs as. Methods that take a `user_id` refuse any
  other uid with `org.freedesktop.DBus.Error.Failed` ("Access denied").
- `RegisterFace` and `DeleteFace` also require the polkit action
  `com.linuxhello.manage-faces` (`auth_self_keep`: the user's own password,
  remembered for a few minutes). `AuthorizeEnrollment` triggers that check on
  its own, so the prompt can appear before the camera starts.
- `Verify` is meant for testing an enrollment (`linux-hello test`). It doesn't
  authenticate anyone to the system and doesn't release any secret.

## JSON payloads

Methods that carry structured data take and return a JSON **string** (`s`),
because the types change more often than the interface. Enumerations use
serde's default representation: a variant without fields is a plain string
(`"NoFaceDetected"`), a variant with fields is an object with a single key
(`{"NoMatch": {...}}`).

`context` is one of `login`, `sudo`, `polkit`, `sddm`, `screenlock` or `test`.
It selects the match threshold.

### `RegisterFace(request_json: s) → s`

Request:

```json
{"user_id": 1000, "context": "sudo", "timeout_ms": 10000, "num_samples": 5}
```

Response:

```json
{"face_id": "face_1000_1730000000", "registered_at": 1730000000, "quality_score": 0.91}
```

Fails if the models aren't installed, no face is detected, or polkit
authorization is refused.

### `DeleteFace(request_json: s)`

```json
{"user_id": 1000, "face_id": "face_1000_1730000000"}
```

`"face_id": null` deletes every face of that user.

### `Verify(request_json: s) → s`

Request:

```json
{"user_id": 1000, "context": "test", "timeout_ms": 5000}
```

Response, one of:

```json
{"Success": {"face_id": "face_1000_1730000000", "similarity_score": 0.83, "used_ir_liveness": true}}
{"NoMatch": {"best_score": 0.41, "threshold": 0.62}}
"NoFaceDetected"
"NoEnrollment"
"NoUsableEmbeddings"
"Cancelled"
{"Error": {"message": "..."}}
```

`NoUsableEmbeddings` means faces are enrolled but none can be read (for
example a TPM-sealed key that no longer unseals). Re-enrolling fixes it.

### `ListFaces(user_id: u) → s`

```json
[{"face_id": "face_1000_1730000000", "user_id": 1000, "quality_score": 0.91,
  "registered_at": 1730000000, "context": "sudo"}]
```

### `CameraInfo() → s`, `EmbeddingEncryptionInfo() → s`, `GetCaptureStatus() → s`

```json
{"has_ir": true}
{"encrypted": true}
{"face_detected": true, "quality_score": 0.78}
```

`has_ir: false` means liveness only uses the weaker RGB heuristic.
`encrypted: false` means no reachable TPM, so embeddings are stored
unencrypted. `GetCaptureStatus` is only meaningful while `StartCaptureStream`
is running.

### Other methods and properties

- `AuthorizeEnrollment(user_id: u)`: runs the polkit check of `RegisterFace`
  on its own.
- `StartCaptureStream(user_id: u, num_frames: u, timeout_ms: t) → s`: live
  preview for an enrollment screen. Blocks until the capture ends and returns
  `"OK"`. It emits **no signal**: poll `GetCaptureStatus` for the detection
  state, and read the frames from the preview server below.
- `StopCaptureStream()`: stops the preview and returns once the camera is free
  again. Call it before `RegisterFace` if a preview is running.
- `Ping() → s`: returns `"pong"`.
- Properties (read-only): `Version` (s), `CameraAvailable` (b), `RootMode` (b),
  `StoragePath` (s).

## Preview frames

While `StartCaptureStream` runs, the daemon serves the frames on
`127.0.0.1:17823`: an MJPEG feed on `/` and the latest frame as a single JPEG
on `/snapshot`. Every request needs `?token=<token>`, read from
`$XDG_RUNTIME_DIR/hello-daemon-mjpeg.token` (mode 0600, regenerated when the
daemon starts).

## Stability

The interface name has no version suffix yet. Changes aim to be additive (new
methods, new JSON fields), so clients should ignore JSON fields they don't
know. Compare against the `Version` property, and diff the XML file between
releases to see what changed.
