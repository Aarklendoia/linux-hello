# Architecture GUI KDE/Wayland - Linux Hello Configuration

## 📋 Vue d'ensemble

Le système de configuration GUI intègre:

1. **hello_daemon** - Capture et détection
2. **linux_hello_config** - Interface utilisateur
3. **D-Bus** - Communication inter-processus

## 🏗️ Architecture Complète

### Modules Créés

```
linux-hello-rust/
├── hello_daemon/
│   └── capture_stream.rs (NOUVEAU)      # Types streaming
│
├── hello_face_core/
│   └── stub_detector.rs (NOUVEAU)       # Détection rapide
│
└── linux_hello_config/ (NOUVEAU)
    ├── main.rs                           # Application principale (Iced)
    ├── ui.rs                             # Écrans de navigation
    ├── preview.rs                        # Affichage caméra
    ├── config.rs                         # Gestion configuration
    └── Cargo.toml                        # Dépendances GUI
```

## 🎨 Écrans Principaux

### 1. **Home (Accueil)**

- Boutons rapides: Enregistrement, Paramètres, Gérer visages
- État du système: Caméra disponible?, Daemon actif?

### 2. **Enrollment (Enregistrement)**

- **Preview en direct** (640×480 RGB)
- **Détection visage**:
  - ✅ Carré vert autour du visage détecté
  - ❌ Aucun visage = pas de carré
- **Barre de progression**: 5/30 frames
- Boutons: Démarrer, Arrêter, Annuler
- Indicateur qualité: Score qualité frame actuelle

### 3. **Settings (Paramètres)**

- Nombre de frames à capturer (default: 30)
- Timeout d'enregistrement (default: 2 min)
- Seuil de confiance détection (0.6)
- Seuil de qualité (0.5)
- Device caméra (/dev/video0)

### 4. **Manage Faces (Gérer Visages)**

- Liste des visages enregistrés
- Supprimer un visage
- Voir les détails (date, qualité)

## 📡 Communication D-Bus

### Signaux Streaming (Daemon → GUI)

```
com.linuxhello.FaceAuth.CaptureProgress
├── frame_number: u32          # 0-indexed
├── total_frames: u32          # 30
├── frame_data: ay             # Vec<u8> RGB
├── width: u32                 # 640
├── height: u32                # 480
├── face_detected: b           # bool
├── face_box: (iiii)           # x, y, w, h optionnel
└── quality_score: d           # f32 (0.0-1.0)
```

### Méthodes D-Bus (GUI → Daemon)

```
com.linuxhello.FaceAuth.StartCapture(
    user_id: u32,
    num_frames: u32,
    timeout_ms: u64
) → OK ou erreur

com.linuxhello.FaceAuth.CancelCapture() → OK

com.linuxhello.FaceAuth.ListFaces(user_id: u32) → [FaceInfo]
```

## 🔄 Flow d'Enregistrement

```
┌──────────────────────────────┐
│  GUI: Écran Enrollment       │
│  Affiche: "Appuyez pour      │
│   commencer"                 │
└──────────────┬───────────────┘
               │ Clic "Démarrer"
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
│  - V4L2 caméra ouvre         │
│  - Boucle 30 frames          │
└──────────────┬───────────────┘
               │ (Boucle)
               ▼
┌──────────────────────────────┐
│  Pour chaque frame:          │
│  1. Capturer V4L2            │
│  2. StubDetector.detect()    │
│  3. Émettre signal D-Bus     │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│  GUI reçoit signal           │
│  1. Affiche la frame RGB     │
│  2. Dessine carré visage     │
│  3. Met à jour barre 5/30    │
└──────────────┬───────────────┘
               │ (Répète x30)
               ▼
┌──────────────────────────────┐
│  Daemon: 30 frames captées   │
│  Sélectionne meilleure       │
│  Extrait embedding           │
│  Sauvegarde                  │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│  GUI: Résultat "Succès!"     │
│  "Visage enregistré"         │
└──────────────────────────────┘
```

## 🎯 Types de Données

### CaptureFrameEvent (Streaming)

```rust
pub struct CaptureFrameEvent {
    pub frame_number: u32,           // 0-29
    pub total_frames: u32,           // 30
    pub frame_data: Vec<u8>,         // RGB 640×480×3
    pub width: u32,                  // 640
    pub height: u32,                 // 480
    pub face_detected: bool,         // Visage?
    pub face_box: Option<FaceBox>,   // Bounding box
    pub quality_score: f32,          // 0.0-1.0
    pub timestamp_ms: u64,           // Depuis début
}
```

### FaceBox

```rust
pub struct FaceBox {
    pub x: u32,                      // Pixel X
    pub y: u32,                      // Pixel Y
    pub width: u32,                  // Largeur box
    pub height: u32,                 // Hauteur box
    pub confidence: f32,             // Confiance détection
}
```

### CaptureState

```rust
pub enum CaptureState {
    Idle,           // Pas de capture
    Waiting,        // En attente de placement
    Capturing,      // Capture en cours
    Completed,      // Succès
    Failed,         // Erreur
    Cancelled,      // Annulé
}
```

## 🎨 Stack Technologique

### Frontend (GUI)

- **Iced** v0.12 - Framework UI cross-platform Rust
  - ✅ Wayland natif
  - ✅ Rendu GPU (wgpu)
  - ✅ Moderne et réactif
- **pixels** v0.13 - Pixel buffer pour rendu RGB frames
- **image** v0.24 - Traitement images

### Backend (Daemon)

- **D-Bus** - Communication inter-processus (zbus)
- **tokio** - Async runtime
- **hello_camera** - Capture V4L2
- **hello_face_core** - Détection (stub pour MVP)

### Détection (MVP)

- **StubDetector** - Détection basée contraste simple
  - Identifie région centrale 640×480
  - Calcule moyenne pixel RGB
  - Retourne si [50, 200] (stub)
  - À remplacer par YOLO/RetinaFace

## 📊 Performance Estimée

| Operation | Latency | CPU | RAM |
|-----------|---------|-----|-----|
| Capture V4L2 | ~33ms (30fps) | ✓ Low | ✓ 1-2MB |
| Détection stub | ~1ms | ✓ Low | ✓ 1MB |
| Rendu frame + box | ~16ms (60fps) | ✓ Low | ✓ 5MB |
| Signal D-Bus | ~5ms | ✓ Low | ✓ 1MB |
| **Total par frame** | **~55ms** | ✓ | ✓ **~8MB** |

**Résultat**: Capture 30 frames en ~1.65 secondes, affichage fluide 30fps

## 🔌 État d'Implémentation

### ✅ Fait

- [x] Types streaming (CaptureFrameEvent, FaceBox, CaptureState)
- [x] StubDetector pour détection rapide
- [x] Module GUI skeleton (Iced)
- [x] Configuration structure
- [x] Modules UI, preview, config
- [x] Compilation complète

### 🚧 À Faire (Prochaines Étapes)

- [ ] Modifier CameraManager pour streaming async
- [ ] Ajouter signaux D-Bus au daemon
- [ ] Implémenter GUI enrollment avec preview
- [ ] Rendu cadre/bounding box sur frame RGB
- [ ] Barre de progression visuelle
- [ ] Tester intégration D-Bus
- [ ] Remplacer StubDetector par détection réelle (YOLO)
- [ ] Écran settings avec enregistrement config
- [ ] Écran manage faces

## 🧪 Tests

Tous les 23 tests passent:

- ✅ 2 tests hello_camera
- ✅ 15 tests hello_daemon (incluant capture_stream)
- ✅ 5 tests hello_face_core (incluant stub_detector)
- ✅ 1 test pam_linux_hello

## 🚀 Prochaines Étapes

1. **Intégration D-Bus complète**
   - Ajouter trait `CaptureSession` au daemon
   - Émettre signaux D-Bus pour chaque frame

2. **Rendu Preview en direct**
   - Décoder frames RGB
   - Dessiner bounding box vert
   - Afficher barre progression

3. **Détection Réelle**
   - Intégrer YOLO détection faciale
   - Optimiser latence
   - Calibrer seuils

4. **Tests et Polish**
   - Tests d'intégration D-Bus
   - Gestion d'erreurs complète
   - UI/UX refinement
