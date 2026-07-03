# Commandes Utiles - Linux Hello Project

## 🏗️ Construction & Compilation

### Build Release (Optimisé)

```bash
cargo build --release
```

**Résultat**: Binaires optimisés dans `target/release/`
**Temps**: ~52 secondes
**Binaires**:

- `hello-daemon` - Daemon PAM et D-Bus
- `linux-hello` - CLI tool
- `linux-hello-config` - GUI (Iced/Wayland)
- `libpam_linux_hello.so` - PAM module

### Build Debug (Développement)

```bash
cargo build
```

**Résultat**: Binaires avec symboles de debug
**Temps**: ~1-2 minutes
**Avantage**: Plus rapide à compiler, meilleur debugging

### Quick Check (Sans Linking)

```bash
cargo check
cargo check --release
```

**Temps**: <1 seconde
**Utilité**: Vérifier les erreurs rapidement sans compiler

---

## 🧪 Tests

### Tous les Tests

```bash
cargo test --release
# Résultat: 35 tests, tous ✅
```

### Tests d'un Crate Spécifique

```bash
cargo test --release -p hello_daemon
cargo test --release -p linux_hello_config
cargo test --release -p hello_face_core
```

### Un Test Unique

```bash
cargo test --release preview::tests::test_get_display_data_with_frame
cargo test --release camera::tests::test_start_capture_stream
```

### Avec Output (Non-capturé)

```bash
cargo test --release -- --nocapture
```

### Couverture de Code (Llvm-cov)

```bash
cargo tarpaulin --release
```

---

## 📦 Installation

### Binaires locaux

```bash
# Release build
cargo build --release

# Dans target/release/
./target/release/hello-daemon    # Lancer le daemon
./target/release/linux-hello     # CLI client
./target/release/linux-hello-config  # GUI
```

### Package Debian (Phase B)

```bash
# Généré dans debian/
dpkg -i libpam-linux-hello_*.deb
dpkg -i linux-hello_*.deb
dpkg -i linux-hello-daemon_*.deb
dpkg -i linux-hello-tools_*.deb
```

---

## 🚀 Exécution

### Daemon

```bash
# Terminal 1 - Lancer le daemon
./target/release/hello-daemon

# Avec logs de debug
RUST_LOG=debug ./target/release/hello-daemon
```

### GUI (KDE/Wayland)

```bash
# Terminal 2 - Lancer la GUI
./target/release/linux-hello-config
```

### CLI Client

```bash
# Terminal 2 - Tester via D-Bus
./target/release/linux-hello \
  --user testuser \
  --timeout 5000 \
  start-capture
```

---

## 📊 Benchmarks

### Performance

```bash
# Build release
time cargo build --release
# → ~52 secondes

# Run tests
time cargo test --release
# → ~2-3 minutes total

# Check only
time cargo check
# → <1 seconde

# Test specific
time cargo test --release camera::
# → ~0.2 secondes
```

---

## 🔍 Debugging

### Voir les Warnings Détaillés

```bash
cargo check 2>&1 | grep "warning:"
```

### Voir les Erreurs Détaillés

```bash
cargo build 2>&1 | grep "error:"
cargo check 2>&1 | grep "error:"
```

### Clippy (Lint Avancé)

```bash
cargo clippy --release
cargo clippy --fix
```

### Documenter & Ouvrir Docs

```bash
cargo doc --open
```

### Voir les Dépendances

```bash
cargo tree
cargo outdated
```

---

## 📝 Documentation

### Générer les Docs

```bash
cargo doc --release --no-deps
```

### Générer et Ouvrir

```bash
cargo doc --open
```

### Voir les Doctests

```bash
cargo test --doc
```

---

## 🧹 Nettoyage

### Supprimer les Artefacts

```bash
cargo clean
```

### Supprimer les Logs

```bash
rm -rf target/
```

### Supprimer les Built Packages

```bash
rm -rf debian/linux-hello*/
rm -rf debian/libpam*/
```

---

## 📋 Commandes Utiles Quotidiennes

### Check & Test Rapide (Development)

```bash
# Moins de 5 secondes
cargo check && cargo test --lib
```

### Build Complet & Test

```bash
# Environ 55 secondes
cargo build --release && cargo test --release
```

### Fix Compiler Warnings

```bash
cargo fix --allow-dirty
cargo fix --allow-dirty --release
```

### Update Dependencies

```bash
cargo update
cargo outdated
```

---

## 🐛 Debugging Avancé

### Avec GDB

```bash
rust-gdb ./target/release/hello-daemon
# Dans gdb:
# (gdb) b main
# (gdb) run
# (gdb) n
```

### Avec LLDB

```bash
lldb ./target/release/hello-daemon
# Dans lldb:
# (lldb) b main
# (lldb) r
# (lldb) n
```

### Avec Valgrind (Memory)

```bash
valgrind --leak-check=full ./target/release/hello-daemon
```

### Trace System Calls

```bash
strace ./target/release/hello-daemon
```

---

## 📦 Distribution

### Build Debian Package

```bash
# Voir Makefile
make build-debian

# Ou manuellement
dpkg-deb --build debian/linux-hello debian/
```

### Check Package Contents

```bash
dpkg -c libpam-linux-hello_*.deb
dpkg -c linux-hello_*.deb
```

### Install from Debian

```bash
sudo dpkg -i *.deb
sudo apt-get install -f  # Fix dependencies
```

---

## 🔧 Configuration

### Debug Logging

```bash
RUST_LOG=debug cargo run --release
RUST_LOG=info,hello_daemon=debug cargo build
```

### Features Flag

```bash
# Build avec fonctionnalités optionnelles
cargo build --release --features "feature1,feature2"
```

---

## 📊 Statistiques du Code

### Compter les Lignes de Code

```bash
# Rust seulement
find . -name "*.rs" -type f | xargs wc -l | tail -1

# Sans dépendances
find . -path ./target -prune -o -name "*.rs" -type f -print | xargs wc -l
```

### Voir les TODO Comments

```bash
grep -r "TODO\|FIXME\|XXX\|HACK" --include="*.rs" .
```

### Complexité Cyclomatique

```bash
cargo install cargo-cyclomatic
cargo cyclomatic
```

---

## 🎯 Git Workflow

### Voir les Changements

```bash
git status
git diff
```

### Commit

```bash
git add .
git commit -m "Phase 3.3: Preview rendering implementation"
git push
```

### Tags

```bash
git tag -a v0.3.3 -m "Phase 3.3 Complete"
git push origin v0.3.3
```

---

## 🚨 Résolution de Problèmes

### Le projet ne compile pas

```bash
# 1. Clean complètement
cargo clean

# 2. Vérifier les dépendances
cargo update

# 3. Reconstruire
cargo build --release
```

### Tests échouent

```bash
# 1. Exécuter un test spécifique
cargo test test_name -- --nocapture

# 2. Voir les logs
RUST_LOG=debug cargo test test_name

# 3. Vérifier la mémoire
valgrind --leak-check=full cargo test
```

### Artefacts de build stale

```bash
# Nettoyer les incrémentaux
cargo clean
cargo build --release

# Ou juste le crate problématique
cargo clean -p hello_daemon
cargo build -p hello_daemon --release
```

---

## 📚 Ressources Utiles

### Documentation Locale

```bash
# Ouvrir les docs du projet
cargo doc --open

# Docs des dépendances
# https://docs.rs/ (web)
```

### Crates.io

- <https://crates.io/crates/iced> - GUI framework
- <https://crates.io/crates/zbus> - D-Bus bindings
- <https://crates.io/crates/v4l> - V4L2 camera
- <https://crates.io/crates/tokio> - Async runtime

---

## ⚙️ Configuration Système (Linux)

### Installer les dépendances de build

```bash
# Ubuntu/Debian
sudo apt-get install \
  build-essential \
  libssl-dev \
  pkg-config \
  libv4l-dev \
  libpam0g-dev \
  libwayland-dev

# Fedora
sudo dnf install \
  gcc \
  openssl-devel \
  pkg-config \
  libv4l-devel \
  pam-devel \
  wayland-devel
```

### Permissions pour Camera

```bash
# Ajouter l'utilisateur au groupe video
sudo usermod -a -G video $USER

# Redémarrer la session ou:
newgrp video
```

### Permission pour PAM

```bash
# Donner les permissions appropriées au module PAM
sudo chown root:root /usr/lib/x86_64-linux-gnu/libpam_linux_hello.so
sudo chmod 755 /usr/lib/x86_64-linux-gnu/libpam_linux_hello.so
```

---

## 📖 Commandes par Scénario

### "Je veux juste vérifier que tout compile"

```bash
cargo check --release
```

### "Je veux vérifier tous les tests"

```bash
cargo test --release
```

### "Je veux builder et exécuter"

```bash
cargo build --release
./target/release/linux-hello-config
```

### "Je veux créer un package Debian"

```bash
make build-debian
# Ou voir debian/rules pour plus de contrôle
```

### "Je veux debugger un test"

```bash
RUST_LOG=debug cargo test test_name -- --nocapture
```

### "Je veux voir si le code est maintien propre"

```bash
cargo fmt --check
cargo clippy --release
```

---

**Version**: 0.3.3
**Dernière mise à jour**: 2026-01-XX
**Pour Phase**: 3.3 (Preview Rendering)
