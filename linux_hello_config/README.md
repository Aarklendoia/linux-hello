# linux_hello_config - Configuration GUI KDE/Wayland

## 📌 Description

Interface graphique native KDE/Wayland pour la configuration et l'enregistrement de visages dans le système Linux Hello.

## ✨ Fonctionnalités

### Actuellement Implémenté (MVP)

- ✅ Application GUI basique avec Iced
- ✅ Navigation entre 4 écrans principaux
- ✅ Structure de configuration
- ✅ Types pour streaming D-Bus

### En Cours de Développement

- 🚧 Écran d'enregistrement avec preview en direct
- 🚧 Détection de visage (stub → YOLO)
- 🚧 Affichage bounding box et barre progression
- 🚧 Communication D-Bus avec daemon

### Futur

- 📋 Écran de paramètres avancés
- 📋 Gestion des visages enregistrés
- 📋 Intégration KDE theme
- 📋 Notifications système

## 🎨 Écrans

### 1. Home (Accueil)

Menu principal avec accès à:

- Enregistrer nouveau visage
- Paramètres
- Gestion des visages

### 2. Enrollment (Enregistrement)

```
┌─────────────────────────────┐
│  Preview Caméra (640×480)   │
│  ┌───────────────────────┐  │
│  │   █████████████████   │  │ ← Frame RGB + détection
│  │   █ O   O        █    │  │   Carré vert si visage
│  │   █       █      █    │  │   détecté
│  │   █     └─┘      █    │  │
│  │   █████████████████   │  │
│  └───────────────────────┘  │
│                             │
│  Progression: ████░░░ 5/30  │ ← Barre progression
│  Qualité: 0.85              │
│                             │
│  [Démarrer]  [Arrêter]     │
└─────────────────────────────┘
```

### 3. Settings (Paramètres)

- Nombre de frames
- Timeout
- Seuils de confiance/qualité
- Device caméra

### 4. Manage Faces (Gestion)

- Liste des visages
- Actions: supprimer, renommer
- Détails: date, qualité

## 🔧 Architecture Technique

### Framework UI

- **Iced 0.12** - Framework UI moderne Rust
  - Cross-platform (Linux, macOS, Windows)
  - Wayland natif
  - GPU-accelerated (wgpu)
  
### Rendu

- **pixels 0.13** - Pixel buffer pour affichage frames RGB
- **image 0.24** - Traitement et manipulation images

### Communication

- **zbus** - D-Bus client
- **tokio** - Async runtime

## 📦 Dépendances Principales

```toml
iced = "0.12"           # Framework UI
pixels = "0.13"         # Pixel rendering
zbus = "4.4"            # D-Bus
tokio = "1.36"          # Async
serde/serde_json        # Serialization
tracing                 # Logging
```

## 🚀 Building & Running

### Compiler

```bash
cargo build --release -p linux_hello_config
```

### Lancer

```bash
./target/release/linux_hello_config
```

### Tests

```bash
cargo test -p linux_hello_config
```

## 📋 Plan d'Implémentation (Phases)

### Phase 1: Foundation ✅

- [x] Structure projet Cargo
- [x] Types streaming et config
- [x] Skeleton GUI avec navigation
- [x] Modules modules ui, preview, config

### Phase 2: Streaming D-Bus 🚧

- [ ] Modifier CameraManager pour streaming async
- [ ] Émettre signaux D-Bus depuis daemon
- [ ] Écouter signaux dans GUI (subscription Iced)
- [ ] Afficher frames en temps réel

### Phase 3: Détection Visage 🚧

- [ ] Intégrer détecteur réel (YOLO ou RetinaFace)
- [ ] Dessiner bounding box sur frames
- [ ] Afficher barre progression
- [ ] Indicateurs qualité/confiance

### Phase 4: Écrans Complets

- [ ] Implémentation complète Settings
- [ ] Implémentation complète Manage Faces
- [ ] Affichage liste visages enregistrés
- [ ] Actions supprimer/éditer

### Phase 5: Polish & Intégration

- [ ] Theme KDE integration
- [ ] Notifications système
- [ ] Gestion erreurs complète
- [ ] Localisation (i18n)
- [ ] Tests d'intégration E2E

## 🎯 État Actuel

- **Compilation**: ✅ Succès (avec warnings mineurs)
- **Tests unitaires**: ✅ 23/23 passant
- **Code organisation**: ✅ Modulaire et extensible
- **GUI opérationnelle**: 🟡 Skeleton seulement
- **D-Bus intégration**: 🔴 À venir

## 📊 Benchmarks

### Performance Ciblée

- Frame rate: 30fps capturée, 30fps affichée
- Latence capture→affichage: <100ms
- Détection: <5ms par frame (stub)
- Mémoire: <50MB pour session capture

## 🤝 Contribution

Pour étendre cette GUI:

1. **Ajouter écran**: Créer module dans `src/screens/`
2. **Ajouter widget**: Implémenter dans `src/ui/`
3. **Modifier comportement**: Éditer `Message` enum
4. **Tester**: Ajouter tests unitaires

## 📚 Références

- [Iced Documentation](https://docs.rs/iced/)
- [D-Bus D-feet Tool](https://wiki.gnome.org/Apps/DFeet) - Inspecter D-Bus
- [RetinaFace](https://github.com/deepinsight/retinaface) - Face detection
- [YOLOv8-Face](https://github.com/akanametov/yolov8-face) - Alternative YOLO

## 📞 Support

Pour des questions ou bugs:

- Consulter `../docs/GUI_ARCHITECTURE.md` pour détails techniques
- Vérifier logs D-Bus: `journalctl -u dbus`
- Tester daemon: `./target/debug/hello-daemon --debug`
