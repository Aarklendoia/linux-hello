# Développement de Linux Hello

Guide rapide pour contribuer au projet.

## ⚡ Démarrage rapide

```bash
# Clone et setup
git clone https://github.com/Aarklendoia/linux-hello.git
cd linux-hello
make dev-setup

# Build et test
make build
make test
make lint
```

## 📋 Prérequis

- Rust 1.70+ (installer via [rustup](https://rustup.rs/))
- Debian/Ubuntu
- Dépendances : `make dev-setup`

## 🏗️ Structure du projet

```
linux-hello/
├── hello_daemon/          # Service de reconnaissance faciale
├── hello_camera/          # Capture et traitement caméra
├── hello_face_core/       # Algorithmes de reconnaissance
├── linux_hello_cli/       # Interface ligne de commande
├── linux_hello_config/    # GUI de configuration (QML)
├── pam_linux_hello/       # Module PAM
├── debian/                # Packaging Debian
├── .github/
│   ├── workflows/         # GitHub Actions CI/CD
│   └── ISSUE_TEMPLATE/    # Templates issues
├── Makefile               # Commandes de dev
├── CONTRIBUTING.md        # Guide de contribution
├── RELEASE.md             # Processus de release
└── CI_CD_INFRASTRUCTURE.md # Documentation CI/CD
```

## 🚀 Commandes principales

```bash
# Development
make build          # Compiler en debug
make release        # Compiler optimisé
make test           # Lancer les tests
make check          # Vérifier rapidement (sans compile)
make fmt            # Formater le code
make lint           # Linter avec clippy
make audit          # Vérifier vulnérabilités

# Debian
make debian         # Compiler les paquets
make deb-install    # Installer les paquets localement
make deb-clean      # Nettoyer les artifacts Debian

# Documentation
make docs           # Générer et ouvrir la doc

# Debug
make daemon         # Lancer le daemon en debug
make camera-test    # Tester la caméra
```

## 🔍 Workflow typique

```bash
# 1. Créer une branche
git checkout -b feature/my-feature

# 2. Faire des modifications
# Éditer les fichiers...

# 3. Tester
make test
make lint

# 4. Committer
git add -A
git commit -m "feat: Description claire"

# 5. Pusher et créer une PR
git push origin feature/my-feature
# Créer une PR sur GitHub
```

## 📦 Packaging Debian

Le projet utilise le format **Debian 3.0 (quilt)**.

### Générer les paquets

```bash
make debian
# Paquets dans ../
ls ../*.deb
```

### Créer un patch

```bash
# Créer et appliquer un patch
quilt new fix-name.patch
quilt add debian/rules
# Éditer le fichier...
quilt refresh

# Lister les patches
quilt series
```

## 🔄 CI/CD automatique

Les workflows GitHub Actions s'exécutent automatiquement :

- **build-debian.yml** : Compile les paquets
- **test.yml** : Lance les tests
- **quality.yml** : Linting et sécurité
- **docs.yml** : Génère la documentation

Voir [CI_CD_INFRASTRUCTURE.md](CI_CD_INFRASTRUCTURE.md) pour plus de détails.

## 📝 Conventions de code

### Rust

```rust
// Doc comments pour les APIs publiques
/// Brief description.
///
/// Longer explanation if needed.
pub fn my_function() {}

// Format avec rustfmt
cargo fmt --all

// Linter avec clippy
cargo clippy --all -- -D warnings
```

### Commits

Format : `<type>: <description>`

Types :
- `feat:` Nouvelle fonctionnalité
- `fix:` Correction de bug
- `docs:` Documentation
- `style:` Formatage
- `refactor:` Refactorisation
- `perf:` Performance
- `test:` Tests
- `chore:` Maintenance

Exemple :
```bash
git commit -m "feat: Add face enrollment API"
```

## 🧪 Tests

```bash
# Tous les tests
cargo test --all

# Test spécifique
cargo test --lib my_test

# Avec output
cargo test -- --nocapture

# Benchmark
cargo bench --all
```

## 📚 Documentation

La documentation Rust est générée automatiquement :

```bash
# Générer et ouvrir
make docs

# Lire un crate spécifique
cargo doc --open --document-private-items
```

## 🐛 Debugging

```bash
# Compiler avec symbols
RUSTFLAGS="-g" cargo build

# Lancer sous un debugger
rust-gdb ./target/debug/hello-daemon

# Lancer avec logs détaillés
RUST_LOG=debug ./target/debug/hello-daemon
```

## 🔐 Sécurité

```bash
# Vérifier les dépendances vulnérables
cargo audit

# Mettre à jour les dépendances
cargo update

# Outdated
cargo outdated
```

## 📖 Plus d'info

- [CONTRIBUTING.md](CONTRIBUTING.md) - Guide complet de contribution
- [RELEASE.md](RELEASE.md) - Processus de release
- [CI_CD_INFRASTRUCTURE.md](CI_CD_INFRASTRUCTURE.md) - Documentation CI/CD
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) - Règles communautaires

## ❓ Questions ?

- Ouvrir une discussion sur GitHub
- Créer une issue
- Consulter la documentation

Merci de contribuer ! 🎉
