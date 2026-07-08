# linux_hello_config - KDE Configuration GUI

## Description

Qt6/QML/Kirigami graphical interface for configuring and enrolling faces in
the Linux Hello system, with support for 10 languages (English, Chinese,
Spanish, Hindi, Arabic, Portuguese, Russian, Japanese, German, French).

## Architecture

The Rust binary (`src/main.rs`) is a thin launcher, not a GUI toolkit host:

1. It resolves the QML entry point (`qml/main.qml`, packaged or from a
   development checkout) and the Qt6 plugin/import paths.
2. It starts a small local TCP control server (bound to `127.0.0.1`, port
   written to `/tmp/linux-hello-ctrl.port` since `Qt.environmentVariable`
   isn't available on this build) that the QML side polls/calls into for
   actions like face registration.
3. It spawns the actual UI via the `qml6` runtime executable, pointed at
   `qml/main.qml`.
4. The control server bridges requests (e.g. `/register-face`) to
   `hello-daemon` by shelling out to `busctl --user call
   com.linuxhello.FaceAuth ...` — there is no native Rust D-Bus client in
   this crate; QML talks to the control server over HTTP-ish requests, and
   the control server talks to the daemon over D-Bus via `busctl`.
5. A per-uid lock file (`/tmp/linux-hello-config-<uid>.lock`) prevents
   opening more than one instance at a time.

## QML files (`qml/`)

- `main.qml` — root window and navigation
- `AppController.qml` — talks to the Rust control server (enrollment,
  status polling)
- `Home.qml`, `Enrollment.qml`, `Settings.qml`, `ManageFaces.qml` — the
  app's screens
- `I18n.qml` + `i18n/*.json` — the translation layer and per-language
  string tables (10 languages, see `docs/I18N_IMPLEMENTATION.md`)

## Building & Running

### Build

```bash
cargo build --release -p linux_hello_config
```

### Run

```bash
./target/release/linux_hello_config
```

Requires `qml6` and the Qt6/Kirigami QML modules listed in
`debian/control` (`qml-qt6`, `qml6-module-org-kde-kirigami`,
`qml6-module-qtquick*`) to be installed.

## Support

For questions or bugs:

- See `../docs/GUI_ARCHITECTURE.md` for technical details
- Check D-Bus logs: `journalctl -u dbus`
- Test the daemon: `./target/debug/hello-daemon --debug`
