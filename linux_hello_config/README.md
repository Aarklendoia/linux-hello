# linux_hello_config - KDE/Wayland Configuration GUI

## 📌 Description

Native KDE/Wayland graphical interface for configuring and enrolling faces in the Linux Hello system.

## ✨ Features

### Currently Implemented (MVP)

- ✅ Basic GUI application with Iced
- ✅ Navigation between 4 main screens
- ✅ Configuration structure
- ✅ Types for D-Bus streaming

### Under Development

- 🚧 Enrollment screen with live preview
- 🚧 Face detection (stub → YOLO)
- 🚧 Bounding box and progress bar display
- 🚧 D-Bus communication with daemon

### Future

- 📋 Advanced settings screen
- 📋 Management of enrolled faces
- 📋 KDE theme integration
- 📋 System notifications

## 🎨 Screens

### 1. Home

Main menu with access to:

- Enroll new face
- Settings
- Face management

### 2. Enrollment

```
┌─────────────────────────────┐
│  Camera Preview (640×480)   │
│  ┌───────────────────────┐  │
│  │   █████████████████   │  │ ← RGB frame + detection
│  │   █ O   O        █    │  │   Green square if face
│  │   █       █      █    │  │   detected
│  │   █     └─┘      █    │  │
│  │   █████████████████   │  │
│  └───────────────────────┘  │
│                             │
│  Progress: ████░░░ 5/30     │ ← Progress bar
│  Quality: 0.85              │
│                             │
│  [Start]  [Stop]           │
└─────────────────────────────┘
```

### 3. Settings

- Number of frames
- Timeout
- Confidence/quality thresholds
- Camera device

### 4. Manage Faces

- List of faces
- Actions: delete, rename
- Details: date, quality

## 🔧 Technical Architecture

### UI Framework

- **Iced 0.12** - Modern Rust UI framework
  - Cross-platform (Linux, macOS, Windows)
  - Native Wayland
  - GPU-accelerated (wgpu)
  
### Rendering

- **pixels 0.13** - Pixel buffer for RGB frame display
- **image 0.24** - Image processing and manipulation

### Communication

- **zbus** - D-Bus client
- **tokio** - Async runtime

## 📦 Main Dependencies

```toml
iced = "0.12"           # UI Framework
pixels = "0.13"         # Pixel rendering
zbus = "4.4"            # D-Bus
tokio = "1.36"          # Async
serde/serde_json        # Serialization
tracing                 # Logging
```

## 🚀 Building & Running

### Build

```bash
cargo build --release -p linux_hello_config
```

### Run

```bash
./target/release/linux_hello_config
```

### Tests

```bash
cargo test -p linux_hello_config
```

## 📋 Implementation Plan (Phases)

### Phase 1: Foundation ✅

- [x] Cargo project structure
- [x] Streaming and config types
- [x] GUI skeleton with navigation
- [x] ui, preview, config modules

### Phase 2: D-Bus Streaming 🚧

- [ ] Modify CameraManager for async streaming
- [ ] Emit D-Bus signals from daemon
- [ ] Listen for signals in GUI (Iced subscription)
- [ ] Display frames in real time

### Phase 3: Face Detection 🚧

- [ ] Integrate real detector (YOLO or RetinaFace)
- [ ] Draw bounding box on frames
- [ ] Display progress bar
- [ ] Quality/confidence indicators

### Phase 4: Complete Screens

- [ ] Full Settings implementation
- [ ] Full Manage Faces implementation
- [ ] Display list of enrolled faces
- [ ] Delete/edit actions

### Phase 5: Polish & Integration

- [ ] KDE theme integration
- [ ] System notifications
- [ ] Complete error handling
- [ ] Localization (i18n)
- [ ] E2E integration tests

## 🎯 Current State

- **Compilation**: ✅ Success (with minor warnings)
- **Unit tests**: ✅ 23/23 passing
- **Code organization**: ✅ Modular and extensible
- **Operational GUI**: 🟡 Skeleton only
- **D-Bus integration**: 🔴 Coming soon

## 📊 Benchmarks

### Target Performance

- Frame rate: 30fps captured, 30fps displayed
- Capture→display latency: <100ms
- Detection: <5ms per frame (stub)
- Memory: <50MB for capture session

## 🤝 Contribution

To extend this GUI:

1. **Add a screen**: Create a module in `src/screens/`
2. **Add a widget**: Implement in `src/ui/`
3. **Modify behavior**: Edit the `Message` enum
4. **Test**: Add unit tests

## 📚 References

- [Iced Documentation](https://docs.rs/iced/)
- [D-Bus D-feet Tool](https://wiki.gnome.org/Apps/DFeet) - Inspect D-Bus
- [RetinaFace](https://github.com/deepinsight/retinaface) - Face detection
- [YOLOv8-Face](https://github.com/akanametov/yolov8-face) - Alternative YOLO

## 📞 Support

For questions or bugs:

- See `../docs/GUI_ARCHITECTURE.md` for technical details
- Check D-Bus logs: `journalctl -u dbus`
- Test the daemon: `./target/debug/hello-daemon --debug`
