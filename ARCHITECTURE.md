# Architecture Diagram & File Structure

## Architecture générale

```
┌─────────────────────────────────────────────────────────────────┐
│                        LOGIN / SUDO / SDDM                       │
│                   (PAM / KScreenLocker / SDDM)                   │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                   /lib/security/pam_linux_hello.so               │
│                    (Module PAM - Rust .so)                       │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ pam_sm_authenticate(context=login/sudo/screenlock/sddm) │   │
│  │ Parse options, call D-Bus Verify, return PAM code       │   │
│  └────────────────────────┬─────────────────────────────────┘   │
└─────────────────────────┼──────────────────────────────────────┘
                         │ D-Bus call (sync)
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│            Daemon d'authentification (hello-daemon)              │
│                    com.linuxhello.FaceAuth                       │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ RegisterFace(user_id, context)                           │   │
│  │   └─> [Camera] → [Detect] → [Extract] → [Store]        │   │
│  │                                                          │   │
│  │ Verify(user_id, context, timeout_ms)                    │   │
│  │   └─> [Camera] → [Detect] → [Extract] → [Compare]      │   │
│  │       → Return MatchResult (Success/NoMatch/NoFace)    │   │
│  │                                                          │   │
│  │ DeleteFace(user_id, face_id?)                           │   │
│  │ ListFaces(user_id)                                      │   │
│  └─────┬──────────────────────────────────────────────┬────┘   │
└────────┼──────────────────────────────────────────────┼────────┘
         │                                              │
         ▼ Camera I/O                                   ▼ Storage
   ┌──────────────────┐                     ┌──────────────────┐
   │  hello_camera    │                     │   SQLite DB      │
   │  ┌────────────┐  │                     │                  │
   │  │ V4L2       │  │                     │ users/           │
   │  │ PipeWire   │  │                     │  faces/          │
   │  │ (future)   │  │                     │   embeddings     │
   │  └────────────┘  │                     │                  │
   └──────────────────┘                     └──────────────────┘
         ▲                                          ▲
         │ Frame                                    │
         │ (RGB/Gray/MJPEG)                         │ Embedding
         │                                          │ JSON
┌────────┴──────────────────────────────────────┘
│              hello_face_core                    │
│  ┌──────────────────────────────────────────┐  │
│  │ FaceDetector trait (detect_faces)        │  │
│  │ EmbeddingExtractor trait (extract)       │  │
│  │ SimilarityMetric trait (compare)         │  │
│  │                                          │  │
│  │ Types:                                   │  │
│  │  - FaceRegion (bbox, landmarks)          │  │
│  │  - Embedding (vector, metadata)          │  │
│  │  - MatchResult (Success/NoMatch/Error)   │  │
│  └──────────────────────────────────────────┘  │
└───────────────────────────────────────────────┘


┌──────────────────────────────────────────────┐
│  Frontend (KCM Qt6 / GUI Kirigami)           │
│  Linux Hello Configuration Module             │
│  ┌────────────────────────────────────────┐  │
│  │ Register face (caméra en temps réel)  │  │
│  │ Delete face                            │  │
│  │ List faces                             │  │
│  │ Config par contexte (login/sudo/etc)  │  │
│  │ Test verification                      │  │
│  └──────────┬───────────────────────────┘  │
│             │ D-Bus calls                  │
│             ▼                              │
│     hello_daemon                           │
└──────────────────────────────────────────────┘
```

## Flux de données: Authentification PAM (suite)

```
User login (PAM authenticate phase)
        │
        ▼
  pam_linux_hello.so
        │
        ├─ pam_get_user() → get username
        ├─ Parse options (context=login, timeout_ms=5000)
        │
        ▼
  D-Bus: /com/linuxhello/FaceAuth.Verify({user_id: 1000, context: "login"})
        │
        ▼
  hello_daemon:
        ├─ Open camera
        ├─ Capture frame (timeout=5000ms)
        ├─ Detect faces (FaceDetector trait)
        │   └─ FaceRegion { bbox, confidence, landmarks }
        ├─ Extract embedding (EmbeddingExtractor trait)
        │   └─ Embedding { vector: Vec<f32>, quality_score }
        ├─ Load stored embeddings for user_id from DB
        ├─ Compare (SimilarityMetric trait)
        │   └─ Calculate similarity_score
        ├─ If similarity >= threshold:
        │   return MatchResult::Success { face_id, similarity_score }
        └─ Else:
            return MatchResult::NoMatch { best_score, threshold }
        │
        ▼
  pam_linux_hello.so receives MatchResult
        │
        ├─ If Success:
        │   ├─ If confirm=true: pam_conv() → ask user "Confirmer? [o/N]"
        │   └─ Return PAM_SUCCESS → PAM succeeds, login/sudo continues
        │
        ├─ If NoMatch/NoFace:
        │   └─ Return PAM_IGNORE → continue with password
        │
        └─ If Error:
            └─ Return PAM_AUTH_ERR → fail, user can retry or fallback

Result: User logged in (face only or face + password fallback)
```

## Structure de fichiers complète

```
linux-hello-rust/
├── Cargo.toml                    (workspace root)
├── Cargo.lock                    (deps lock, après first build)
├── README.md                     (vue d'ensemble)
├── DESIGN.md                     (spec D-Bus/PAM détaillée)
├── QUICKSTART.md                 (ce fichier)
├── TODO.md                       (roadmap)
├── ARCHITECTURE.md               (ce fichier)
├── .gitignore
│
├── hello_face_core/              (crate lib)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                (traits, types, erreurs)
│       └── (futures: backends/)
│
├── hello_camera/                 (crate lib)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                (abstraction caméra, V4L2 stub)
│       └── (futures: v4l2_impl/, pipewire_impl/)
│
├── hello_daemon/                 (crate lib + bin)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                (FaceAuthDaemon, DaemonConfig)
│   │   ├── main.rs               (CLI entry point)
│   │   ├── dbus_interface.rs     (API D-Bus types/interface)
│   │   ├── (futures: storage.rs, dbus_server.rs, camera_manager.rs)
│   │   └── (future: migrations/sqlite/)
│   │
│   └── (futures: tests/integration/, examples/)
│
├── pam_linux_hello/              (crate lib -> .so)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                (pam_sm_authenticate, options parsing)
│       └── (futures: dbus_client.rs, conversation.rs)
│
├── linux_hello_cli/              (crate bin)
│   ├── Cargo.toml
│   └── src/
│       └── main.rs               (CLI: daemon, enroll, verify, etc.)
│
└── (futures dirs)
    ├── docs/                     (documentation complète)
    ├── packaging/                (spec RPM, deb, AUR)
    ├── systemd/                  (hello-daemon.service)
    ├── pam.d/                    (config PAM examples)
    ├── kde-kcm/                  (future: Qt6 module)
    └── tests/integration/        (E2E tests)
```

## Dépendances externes

### Build-time
- rustc 1.85+
- cargo

### Runtime
- Linux kernel avec V4L2 ou PipeWire (caméra)
- PAM (glibc) pour intégration authentication
- D-Bus (systemd)
- SQLite 3.x (storage)
- OpenSSL (sqlx/native-tls)

### Crates Rust principales
| Crate | Rôle | Notes |
|-------|------|-------|
| tokio | Async runtime | Tokio multi-thread |
| zbus | D-Bus client/server | v4.4 |
| serde/serde_json | Sérialisation | JSON pour D-Bus |
| sqlx | SQLite ORM async | Migrationsincluded |
| tracing | Logging | Structured logging |
| clap | CLI parsing | derive macros |
| pam-sys | PAM bindings | C bindings |
| thiserror | Error types | Derive error impl |

## Points clés de design

### 1. Modularité
- Chaque crate peut être testée seule
- Traits pour backend abstraction
- Types génériques quand possible

### 2. Sécurité
- PAM gère l'interface utilisateur
- Daemon gère les données sensibles
- D-Bus + ACL pour permission
- Stockage SQLite avec umask strict

### 3. Extensibilité
- Traits EmbeddingExtractor, FaceDetector, SimilarityMetric
- Backend caméra: V4L2 + PipeWire future
- Options PAM pour configuration flexible
- MatchResult enum pour extensibilité

### 4. Testabilité
- 100% composants mockables
- Pas de I/O bloquante jusqu'au daemon
- Tests unitaires pour types/logique
- Fixtures pour caméra/embeddings futurs

## Checklist d'implémentation MVP

- [x] Traits abstraits (hello_face_core)
- [x] Abstraction caméra stub (hello_camera)
- [x] Types daemon et API D-Bus (hello_daemon)
- [x] Module PAM skeleton (pam_linux_hello)
- [x] CLI de test (linux_hello_cli)
- [x] Compilation et tests unitaires
- [ ] Stockage SQLite (hello_daemon/storage.rs)
- [ ] D-Bus exposition réelle (hello_daemon/dbus_server.rs)
- [ ] Appels D-Bus depuis PAM (pam_linux_hello/dbus_client.rs)
- [ ] Backend détection (hello_face_core/backends/)
- [ ] Intégration PAM services (login, sudo, kde, sddm)
- [ ] KCM Qt6 (future: kde-kcm/)
- [ ] Packaging et systemd service
- [ ] E2E tests et validation

## Références utiles

- [PAM Programmer's Manual](http://www.linux-pam.org/Linux-PAM-html/)
- [D-Bus Specification](https://dbus.freedesktop.org/doc/dbus-daemon.1.html)
- [Rust FFI Book](https://doc.rust-lang.org/nomicon/ffi.html)
- [KDE/Plasma Integration](https://develop.kde.org/docs/)
- [systemd Service](https://man.archlinux.org/man/systemd.service.5)

