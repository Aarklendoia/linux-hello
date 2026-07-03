# KDE/Wayland GUI Architecture - Linux Hello Configuration

## 📋 Overview

The GUI configuration system integrates:

1. **hello_daemon** - Capture and detection
2. **linux_hello_config** - User interface
3. **D-Bus** - Inter-process communication

## 🏗️ Full Architecture

### Modules Created

```
linux-hello-rust/
├── hello_daemon/
│   └── capture_stream.rs (NEW)      # Streaming types
│
├── hello_face_core/
│   └── stub_detector.rs (NEW)       # Fast detection
│
└── linux_hello_config/ (NEW)
    ├── main.rs                           # Main application (Iced)
    ├── ui.rs                             # Navigation screens
    ├── preview.rs                        # Camera display
    ├── config.rs                         # Configuration management
    └── Cargo.toml                        # GUI dependencies
```

## 🎨 Main Screens

### 1. **Home**

- Quick buttons: Enrollment, Settings, Manage faces
- System state: Camera available?, Daemon active?

### 2. **Enrollment**

- **Live preview** (640×480 RGB)
- **Face detection**:
  - ✅ Green square around the detected face
  - ❌ No face = no square
- **Progress bar**: 5/30 frames
- Buttons: Start, Stop, Cancel
- Quality indicator: Current frame quality score

### 3. **Settings**

- Number of frames to capture (default: 30)
- Enrollment timeout (default: 2 min)
- Detection confidence threshold (0.6)
- Quality threshold (0.5)
- Camera device (/dev/video0)

### 4. **Manage Faces**

- List of enrolled faces
- Delete a face
- View details (date, quality)

## 📡 D-Bus Communication

### Streaming Signals (Daemon → GUI)

```
com.linuxhello.FaceAuth.CaptureProgress
├── frame_number: u32          # 0-indexed
├── total_frames: u32          # 30
├── frame_data: ay             # Vec<u8> RGB
├── width: u32                 # 640
├── height: u32                # 480
├── face_detected: b           # bool
├── face_box: (iiii)           # x, y, w, h optional
└── quality_score: d           # f32 (0.0-1.0)
```

### D-Bus Methods (GUI → Daemon)

```
com.linuxhello.FaceAuth.StartCapture(
    user_id: u32,
    num_frames: u32,
    timeout_ms: u64
) → OK or error

com.linuxhello.FaceAuth.CancelCapture() → OK

com.linuxhello.FaceAuth.ListFaces(user_id: u32) → [FaceInfo]
```

## 🔄 Enrollment Flow

```
┌──────────────────────────────┐
│  GUI: Enrollment Screen      │
│  Displays: "Press to         │
│   start"                     │
└──────────────┬───────────────┘
               │ Click "Start"
               ▼
┌──────────────────────────────┐
│  GUI → D-Bus                 │
│  StartCapture(user_id=1000,  │
│   num_frames=30, ...)        │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│  Daemon: capture_frames()     │
│  - V4L2 camera opens         │
│  - 30-frame loop              │
└──────────────┬───────────────┘
               │ (Loop)
               ▼
┌──────────────────────────────┐
│  For each frame:              │
│  1. Capture V4L2              │
│  2. StubDetector.detect()     │
│  3. Emit D-Bus signal         │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│  GUI receives signal         │
│  1. Displays the RGB frame   │
│  2. Draws the face square    │
│  3. Updates the bar 5/30     │
└──────────────┬───────────────┘
               │ (Repeats x30)
               ▼
┌──────────────────────────────┐
│  Daemon: 30 frames captured  │
│  Selects the best one         │
│  Extracts embedding           │
│  Saves                        │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│  GUI: Result "Success!"      │
│  "Face enrolled"              │
└──────────────────────────────┘
```

## 🎯 Data Types

### CaptureFrameEvent (Streaming)

```rust
pub struct CaptureFrameEvent {
    pub frame_number: u32,           // 0-29
    pub total_frames: u32,           // 30
    pub frame_data: Vec<u8>,         // RGB 640×480×3
    pub width: u32,                  // 640
    pub height: u32,                 // 480
    pub face_detected: bool,         // Face?
    pub face_box: Option<FaceBox>,   // Bounding box
    pub quality_score: f32,          // 0.0-1.0
    pub timestamp_ms: u64,           // Since start
}
```

### FaceBox

```rust
pub struct FaceBox {
    pub x: u32,                      // Pixel X
    pub y: u32,                      // Pixel Y
    pub width: u32,                  // Box width
    pub height: u32,                 // Box height
    pub confidence: f32,             // Detection confidence
}
```

### CaptureState

```rust
pub enum CaptureState {
    Idle,           // No capture
    Waiting,        // Waiting for positioning
    Capturing,      // Capture in progress
    Completed,      // Success
    Failed,         // Error
    Cancelled,      // Cancelled
}
```

## 🎨 Technology Stack

### Frontend (GUI)

- **Iced** v0.12 - Cross-platform Rust UI framework
  - ✅ Native Wayland
  - ✅ GPU rendering (wgpu)
  - ✅ Modern and reactive
- **pixels** v0.13 - Pixel buffer for RGB frame rendering
- **image** v0.24 - Image processing

### Backend (Daemon)

- **D-Bus** - Inter-process communication (zbus)
- **tokio** - Async runtime
- **hello_camera** - V4L2 capture
- **hello_face_core** - Detection (stub for MVP)

### Detection (MVP)

- **StubDetector** - Simple contrast-based detection
  - Identifies the central 640×480 region
  - Computes average RGB pixel
  - Returns match if in [50, 200] (stub)
  - To be replaced with YOLO/RetinaFace

## 📊 Estimated Performance

| Operation | Latency | CPU | RAM |
|-----------|---------|-----|-----|
| V4L2 Capture | ~33ms (30fps) | ✓ Low | ✓ 1-2MB |
| Stub detection | ~1ms | ✓ Low | ✓ 1MB |
| Frame + box rendering | ~16ms (60fps) | ✓ Low | ✓ 5MB |
| D-Bus signal | ~5ms | ✓ Low | ✓ 1MB |
| **Total per frame** | **~55ms** | ✓ | ✓ **~8MB** |

**Result**: Captures 30 frames in ~1.65 seconds, smooth 30fps display

## 🔌 Implementation Status

### ✅ Done

- [x] Streaming types (CaptureFrameEvent, FaceBox, CaptureState)
- [x] StubDetector for fast detection
- [x] GUI skeleton module (Iced)
- [x] Configuration structure
- [x] UI, preview, config modules
- [x] Full compilation

### 🚧 To Do (Next Steps)

- [ ] Modify CameraManager for async streaming
- [ ] Add D-Bus signals to the daemon
- [ ] Implement GUI enrollment with preview
- [ ] Render frame/bounding box on the RGB frame
- [ ] Visual progress bar
- [ ] Test D-Bus integration
- [ ] Replace StubDetector with real detection (YOLO)
- [ ] Settings screen with config saving
- [ ] Manage faces screen

## 🧪 Tests

All 23 tests pass:

- ✅ 2 hello_camera tests
- ✅ 15 hello_daemon tests (including capture_stream)
- ✅ 5 hello_face_core tests (including stub_detector)
- ✅ 1 pam_linux_hello test

## 🚀 Next Steps

1. **Full D-Bus Integration**
   - Add `CaptureSession` trait to the daemon
   - Emit D-Bus signals for each frame

2. **Live Preview Rendering**
   - Decode RGB frames
   - Draw green bounding box
   - Display progress bar

3. **Real Detection**
   - Integrate YOLO face detection
   - Optimize latency
   - Calibrate thresholds

4. **Tests and Polish**
   - D-Bus integration tests
   - Full error handling
   - UI/UX refinement
