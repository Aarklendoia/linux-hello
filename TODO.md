# TODO - Linux Hello Development Roadmap

## Phase 1: MVP Core et infrastructure basique

### Tâche 1.1: Complétez les imports manquants
- [ ] Ajouter `libc` au workspace dependencies pour accès UID/GID
- [ ] Vérifier que `parking_lot` est disponible pour `SharedCamera`

### Tâche 1.2: Implémentation V4L2 réelle
- [ ] Utiliser le crate `v4l` pour binding V4L2 propre
- [ ] Implémenter `CameraBackend::capture()` avec vraie capture
- [ ] Gérer formats: RGB8, Grayscale, MJPEG
- [ ] Tester avec `/dev/video0`

### Tâche 1.3: Backend détection/embedding (STUB pour MVP)
- [ ] Créer `hello_face_core::detectors::StubDetector` pour tests
- [ ] Créer `hello_face_core::extractors::StubExtractor` pour tests
- [ ] Créer `hello_face_core::similarity::CosineSimilarity`
- [ ] Permettre compiler sans dépendances externes complexes

### Tâche 1.4: CLI complètement fonctionnelle (sans daemon D-Bus)
- [ ] Implémenter `command_camera()` avec vraie caméra
- [ ] Tester capture locale sans daemon
- [ ] Ajouter commande de test local: `linux-hello test-detect`

---

## Phase 2: Daemon et stockage

### Tâche 2.1: Stockage SQLite
- [ ] Créer migrations SQLite (schema: users, faces, embeddings)
- [ ] Implémenter `FaceRepository` pour CRUD embeddings
- [ ] Tests: créer/lire/supprimer embeddings

### Tâche 2.2: Daemon D-Bus réel
- [ ] Connecter `FaceAuthService` à une vraie instance D-Bus
- [ ] Implémenter `register_face()` avec capture caméra + extraction
- [ ] Implémenter `delete_face()` et `verify()`
- [ ] Implémenter `list_faces()`
- [ ] Gestion des erreurs D-Bus propre

### Tâche 2.3: Permissions et ACL
- [ ] Vérifier UID lors des appels D-Bus
- [ ] Empêcher user A de gérer visage de user B
- [ ] Cas root: permettre gérer n'importe quel utilisateur

### Tâche 2.4: Systemd service
- [ ] Créer `hello-daemon.service` pour démarrage automatique
- [ ] Options: `--user` pour mode user-daemon, sans flag = mode root
- [ ] Socket activation optionnelle

---

## Phase 3: Module PAM intégration

### Tâche 3.1: Appels D-Bus depuis PAM
- [ ] Implémenter `call_daemon()` pour appels D-Bus synchrones
- [ ] Gérer timeouts PAM
- [ ] Convertir UID en UID numérique

### Tâche 3.2: Conversation PAM
- [ ] Implémenter `pam_conv_send()` correctement
- [ ] Afficher prompts: "En attente de reconnaissance faciale...", "Confirmer [o/N]?"
- [ ] Gérer réponses utilisateur

### Tâche 3.3: Test PAM custom
- [ ] Créer service PAM test: `/etc/pam.d/linux-hello-test`
- [ ] Compiler module `.so`
- [ ] Tester avec `pamtester` ou `su`

### Tâche 3.4: Intégration services standards
- [ ] `/etc/pam.d/login` (TTY)
- [ ] `/etc/pam.d/sudo`
- [ ] `/etc/pam.d/kde` (KScreenLocker)
- [ ] `/etc/pam.d/sddm` (SDDM)

---

## Phase 4: KDE/Plasma UI

### Tâche 4.1: KCM (KDE Control Module)
- [ ] Nouveau projet Qt6 simple
- [ ] Interface: liste visages, boutons Enregistrer/Supprimer/Tester
- [ ] Appeler daemon D-Bus via QDBus

### Tâche 4.2: Enregistrement UI
- [ ] Widget caméra en temps réel
- [ ] Messages: "Regardez la caméra...", "Détection OK", "Trop sombre", etc.
- [ ] Progress bar (nombre samples restants)
- [ ] Confirmation finale

### Tâche 4.3: Configuration par contexte
- [ ] Fichier `~/.config/linux-hello/config.toml`
- [ ] Checkboxes: `[x] Login, [ ] Sudo, [x] Screenlock, [ ] SDDM`
- [ ] Module PAM lit ce fichier pour savoir si contexte activé

---

## Phase 5: Intégrations avancées

### Tâche 5.1: SDDM UI
- [ ] Plugin QML SDDM optionnel pour afficher flux caméra au login
- [ ] PAM reste le moteur principal
- [ ] Plugin = UI sugar, pas logique

### Tâche 5.2: Sudo avancé
- [ ] Confirmation graphique via `pam_conv`
- [ ] CLI: afficher prompt, attendre réponse, confirmer ou annuler
- [ ] Intégration avec `kdesu` si désiré

### Tâche 5.3: Polkit/pkexec
- [ ] Examiner comment Kubuntu 25.10 configure polkit
- [ ] Optionnel: ajouter module dans service PAM polkit

### Tâche 5.4: Tests d'intégration
- [ ] Suite de tests VM/container
- [ ] Scénarios: login, sudo, screenlock, SDDM
- [ ] Fallback password partout

---

## Qualité et release

### Tâche 6.1: Tests unitaires
- [ ] Couvrir 80%+ du code core
- [ ] Mocking pour caméra et daemon
- [ ] Tests sérialisation/désérialisation

### Tâche 6.2: Logging et debug
- [ ] Tracing cohérent partout
- [ ] Logs audit: qui s'authentifie, quand, résultat
- [ ] Option `--debug` générale

### Tâche 6.3: Documentation
- [ ] Docs rustdoc complètes pour APIs publiques
- [ ] Guides installation par distro
- [ ] Troubleshooting courant

### Tâche 6.4: Packaging
- [ ] Spec RPM
- [ ] Dépôt AUR
- [ ] PPA Ubuntu
- [ ] Flatpak optionnel

---

## Notes

**Dépendances à résoudre:**
- Ajouter `libc`, `parking_lot` au workspace
- Backend détection: ONNX Runtime (future) ou stub
- V4L2 binding: vérifier qualité crate `v4l`

**Blockers identifiés:**
- Module PAM Rust compile ok mais pas encore testé
- D-Bus nécessite connaissance des binding zbus (doc à étudier)
- SDDM sans UI supplémentaire = c'est OK, PAM suffit

**Architecture figée:**
- 4 crates + CLI + future KCM
- D-Bus comme IPC central
- PAM comme glue layer
- Stockage local + ACL utilisateur
