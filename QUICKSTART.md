# Quick Start Guide

## 🚀 Compilation

Préalable: Rust 1.85+ (rustup)

```bash
cd /home/edtech/Documents/linux-hello-rust

# Mode debug (rapide)
cargo build --all

# Mode release (optimisé, pour deployment)
TMPDIR=/home/edtech/tmp cargo build --all --release
```

Résultats:
- Daemon: `target/release/hello-daemon`
- CLI: `target/release/linux-hello`
- Module PAM: `target/release/libpam_linux_hello.so`

## 🧪 Tests

Exécuter tous les tests unitaires:
```bash
TMPDIR=/home/edtech/tmp cargo test --all --lib
```

Résultat attendu: ~10 tests passants, 0 failures

## 🛠️ Démarrage du daemon (mode développement)

```bash
# Terminal 1: Lancer le daemon
cargo run -p linux_hello_cli -- daemon --debug

# Terminal 2: Tester la caméra
cargo run -p linux_hello_cli -- camera --duration 5

# Terminal 2: Enregistrer un visage (non fonctionnel yet)
cargo run -p linux_hello_cli -- enroll 1000 --samples 3

# Terminal 2: Vérifier (non fonctionnel yet)
cargo run -p linux_hello_cli -- verify 1000
```

## 📦 Structure du projet

```
.
├── Cargo.toml          (workspace root)
├── README.md           (vue d'ensemble)
├── DESIGN.md           (spécifications D-Bus/PAM détaillées)
├── TODO.md             (roadmap complet)
├── .gitignore
│
├── hello_face_core/    (lib - traits, types)
├── hello_camera/       (lib - abstraction caméra)
├── hello_daemon/       (lib + bin - service D-Bus)
├── pam_linux_hello/    (lib -> .so - module PAM)
└── linux_hello_cli/    (bin - CLI de test)
```

## 🏗️ Étapes suivantes prioritaires

1. **Phase 1 terminée** ✓ - Architecture de base
2. **Phase 2** - Implémentation réelle:
   - [ ] Stockage SQLite dans hello_daemon
   - [ ] D-Bus exposition réelle (zbus)
   - [ ] Appel caméra réelle (V4L2 binding complet)
   - [ ] Backend détection (stub ou ONNX)

3. **Phase 3** - Intégration PAM:
   - [ ] Appels D-Bus depuis module PAM
   - [ ] Tests PAM custom
   - [ ] Intégration login/sudo/kde

4. **Phase 4+** - KDE/Plasma, SDDM, hardening

Voir [TODO.md](TODO.md) pour la liste complète avec dépendances.

## 🔧 Configuration du workspace

- **Edition**: 2021
- **Rust**: 1.85+
- **Dependencies**: tokio, zbus, serde, sqlx, tracing, etc.
- **Profiles**: Release optimisé pour .so (lto=true)

## 📚 Documentation

- **[README.md](README.md)** - Architecture générale
- **[DESIGN.md](DESIGN.md)** - Spec D-Bus/PAM complète
- **[TODO.md](TODO.md)** - Roadmap et tâches
- **Code comments** - Rustdoc + inline comments

## ⚠️ Limitations actuelles (MVP)

- V4L2 en mode stub (retourne frame vide)
- Détection/embedding en mode stub
- D-Bus pas encore exposée
- Stockage en RAM uniquement
- PAM non connectée au daemon
- Pas de UI KDE

Ces limitations sont intentionnelles: le MVP valide l'architecture.
Phase 2 ajoute les implémentations réelles progressivement.

## 🔗 Prochains fichiers à créer

1. **hello_daemon/migrations/001_init.sql** - Schéma SQLite
2. **hello_daemon/src/storage.rs** - Repository SQLite
3. **hello_daemon/src/dbus_server.rs** - Exposition D-Bus réelle
4. **pam_linux_hello/src/dbus_client.rs** - Client D-Bus depuis PAM
5. **tests/integration/** - Tests E2E

## 💡 Notes de développement

- Toutes les crates compilent et testent ✓
- Warnings peuvent être ignorés (imports inutilisés en stub)
- Utilisez `TMPDIR=/home/edtech/tmp` si compilation échoue sur /tmp
- Les constantes PAM sont en dur (utils/pam_constants.h si besoin évolution)
- Architecture est figée, on peut commencer l'implémentation

Bonne chance! 🚀
