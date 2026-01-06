# Linux Hello - Système d'authentification par reconnaissance faciale

Architecture propre et modulaire d'un système d'authentification faciale pour Linux/KDE Plasma.

## 🏗️ Architecture

Quatre composants principaux:

### 1. **hello_face_core** - Moteur de reconnaissance
- Lib Rust indépendante
- Traits d'abstraction: `FaceDetector`, `EmbeddingExtractor`, `SimilarityMetric`
- Types: `FaceRegion`, `Embedding`, `MatchResult`
- Backend-agnostique (ONNX Runtime, TensorFlow, ncnn, etc. à ajouter)

### 2. **hello_camera** - Abstraction caméra
- Trait `CameraBackend` pour implémentations multi-backend
- Actuellement: V4L2 (simple)
- Future: PipeWire pour Wayland/Kubuntu 25.10
- Type `Frame` générique avec support RGB/Grayscale/MJPEG

### 3. **hello_daemon** - Service D-Bus
- Daemon tournant par utilisateur ou root
- Interface D-Bus: `com.linuxhello.FaceAuth`
- Méthodes:
  - `RegisterFace` - enregistrer un visage
  - `DeleteFace` - supprimer un visage
  - `Verify` - vérifier l'identité
  - `ListFaces` - lister les visages enregistrés
- Stockage: `~/.local/share/linux-hello/faces.db` (mode user) ou `/var/lib/linux-hello/` (mode root)

### 4. **pam_linux_hello** - Module PAM
- Librairie partagée compilée: `libpam_linux_hello.so`
- Implémente `pam_sm_authenticate`
- Appelle le daemon D-Bus pour vérifier
- Options configurables: `context`, `timeout_ms`, `similarity_threshold`, `confirm`
- Gestion PAM conversation pour prompts utilisateur

### 5. **linux_hello_cli** - CLI de test/développement
- Commandes: `daemon`, `enroll`, `verify`, `list`, `delete`, `camera`
- Permet tester sans PAM pendant le développement

## 📊 Plan de développement

### Phase 1: MVP Core ✓
- [x] Structures core (FaceRegion, Embedding, traits)
- [x] Abstraction caméra
- [x] Types daemon et API D-Bus
- [ ] Implémenter capture caméra réelle (V4L2 binding)
- [ ] Ajouter backend détection (ONNX ou stub)

### Phase 2: Daemon fonctionnel
- [ ] Stockage SQLite des embeddings
- [ ] Exposition réelle D-Bus
- [ ] Appels caméra depuis le daemon
- [ ] Extraction embeddings

### Phase 3: Module PAM intégré
- [ ] Appels D-Bus depuis PAM
- [ ] Gestion conversation PAM
- [ ] Tests service PAM custom
- [ ] Intégration login standard

### Phase 4: KDE/Plasma
- [ ] KCM (KDE Control Module) pour config
- [ ] Enregistrement graphique
- [ ] Intégration KScreenLocker
- [ ] Config par contexte

### Phase 5: SDDM et sudo avancé
- [ ] SDDM PAM integration
- [ ] Confirmation sudo (pam_conv)
- [ ] Plugin QML SDDM optionnel
- [ ] Polkit/pkexec support

## 🚀 Démarrage

### Build
```bash
cargo build --release

# Chaque crate peut être buildée séparément
cargo build -p hello_face_core --release
cargo build -p hello_camera --release
cargo build -p hello_daemon --release
cargo build -p pam_linux_hello --release
cargo build -p linux_hello_cli --release
```

### Installation PAM (une fois implémenté)
```bash
sudo cp target/release/libpam_linux_hello.so /lib/security/
```

### Usage CLI
```bash
# Tester caméra
cargo run -p linux_hello_cli -- camera --duration 5

# Daemon (mode test)
cargo run -p linux_hello_cli -- daemon --debug

# Enregistrement (quand daemon actif)
cargo run -p linux_hello_cli -- enroll 1000 --samples 3
```

## 📐 Configuration PAM (exemple)

Pour SDDM:
```text
# /etc/pam.d/sddm
auth   sufficient   pam_linux_hello.so context=sddm timeout_ms=5000
auth   include      system-login
```

Pour sudo:
```text
# /etc/pam.d/sudo
auth   sufficient   pam_linux_hello.so context=sudo confirm=true
auth   include      system-auth
```

Pour KScreenLocker:
```text
# /etc/pam.d/kde
auth   sufficient   pam_linux_hello.so context=screenlock
auth   include      system-login
```

## 🔐 Permissions et sécurité

- **Stockage**: `~/.local/share/linux-hello/faces.db` (0700, user only)
  ou `/var/lib/linux-hello/users/$UID/faces.db` (0700, root:root)
- **D-Bus ACL**: Chaque utilisateur ne peut gérer que son propre visage
- **PAM**: Appels non-bloquants quand possible, timeout défini
- **Enregistrement**: Nécessite confirmation (prompts graphiques via PAM)

## 📚 Structure crates

```
linux-hello-rust/
├── Cargo.toml (workspace)
├── hello_face_core/     (lib)
├── hello_camera/        (lib)
├── hello_daemon/        (lib + bin)
├── pam_linux_hello/     (lib -> .so)
├── linux_hello_cli/     (bin)
└── README.md
```

## ⚙️ Dépendances principales

- **Async**: tokio 1.36
- **D-Bus**: zbus 4.0
- **PAM**: pam-sys 0.5
- **Serialization**: serde + serde_json
- **Storage**: sqlx + sqlite
- **Vision** (future): ndarray, image, onnxruntime-rs

## 📝 Notes de conception

### API D-Bus générique
Les appels au daemon utilisent JSON pour sérialisation, permettant:
- Évolution future sans breaking changes
- Flexibilité contexte (login/sudo/screenlock/sddm)
- Logging/audit détaillé

### Module PAM élégant
- Pas de dépendance UI
- Configuration par options (suffisant, required, optional)
- Fallback gracieux vers password
- Timeout bornés

### Modularité
Chaque crate peut être testée/utilisée indépendamment:
- `hello_face_core` = pure vision (testable hors système)
- `hello_camera` = I/O caméra (mockable)
- `hello_daemon` = orchestration (testable avec daemon fictif)
- `pam_linux_hello` = PAM glue layer (simple)

## 🔄 Prochaines étapes concrètes

1. **Ajouter V4L2 binding réel** dans `hello_camera`
2. **Implémenter backend détection** (ou binding ONNX)
3. **Implémentation D-Bus réelle** dans `hello_daemon`
4. **Stockage SQLite** pour embeddings
5. **Appels D-Bus depuis PAM**

Voir `TODO.md` pour détails.
