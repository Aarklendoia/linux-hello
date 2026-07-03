# Contributing to Linux Hello

Merci de contribuer à Linux Hello ! Voici comment procéder.

## Configuration du développement

### Prérequis

- Rust 1.70+ (stable)
- Debian/Ubuntu with build-essential
- libssl-dev, libpam0g-dev, pkg-config
- Qt 6 development libraries
- Kirigami design system

### Installation de l'environnement

```bash
# Clone le repository
git clone https://github.com/Aarklendoia/linux-hello.git
cd linux-hello

# Installez les dépendances système
sudo apt-get install build-essential libssl-dev libpam0g-dev pkg-config \
  qt6-base-dev qml6-module-qtcore libkf6kirigami-dev

# Buildez le projet
cargo build --release
```

## Processus de contribution

### 1. Créez une branche

```bash
git checkout -b feature/my-feature
# ou
git checkout -b fix/my-fix
```

### 2. Faites vos modifications

- Respectez le style de code (rustfmt)
- Écrivez des tests pour les nouvelles fonctionnalités
- Mettez à jour la documentation

### 3. Testez votre code

```bash
# Tests
cargo test --all

# Linting
cargo clippy --all -- -D warnings

# Formatting
cargo fmt --all

# Audit des dépendances
cargo audit
```

### 4. Committez avec des messages clairs

```bash
git commit -m "feat: Add authentication support"
```

Format des messages de commit :
- `feat:` pour les nouvelles fonctionnalités
- `fix:` pour les corrections de bugs
- `docs:` pour la documentation
- `style:` pour les changements de style
- `refactor:` pour les refactorisations
- `perf:` pour les optimisations de performance
- `test:` pour les tests
- `chore:` pour les tâches de maintenance

### 5. Poussez et créez une Pull Request

```bash
git push origin feature/my-feature
```

Puis créez une PR sur GitHub. La CI/CD vérifiera automatiquement :
- Les tests passent
- Le code est bien formaté
- Les linters passent
- Les vulnérabilités ne sont pas présentes

## Format Debian

Le projet utilise le format Debian 3.0 (quilt) pour les paquets Debian.

### Structure

```
debian/
├── source/
│   └── format (3.0 quilt)
├── patches/ (si nécessaire)
├── rules
├── control
├── postinst
└── ...
```

### Ajouter un patch

```bash
quilt new my-fix.patch
quilt add file-to-modify
# Modifiez le fichier
quilt refresh
```

## Paquets Debian

Pour construire localement :

```bash
cd linux-hello
dpkg-buildpackage -us -uc -b
```

Les paquets générés seront dans le répertoire parent.

## Documentation

La documentation est générée avec cargo-doc et disponible en :

- README.md - Introduction
- docs/QUICKSTART.md - Guide de démarrage rapide
- docs/INTEGRATION_GUIDE.md - Intégration dans les systèmes
- docs/PAM_MODULE.md - Documentation du module PAM

## Rapports de bugs

Créez une issue GitHub avec :
- Version de Linux Hello (`linux-hello --version`)
- Système d'exploitation et version
- Étapes pour reproduire
- Comportement attendu vs réel
- Logs pertinents

## Conventions de code

### Rust

- Utilisez `rustfmt` pour le formatage
- Suivez les règles clippy
- Écrivez des docs comments pour les APIs publiques
- Nommez les variables de manière explicite

### QML

- Indentation: 4 espaces
- Nommez les IDs en camelCase
- Groupez les propriétés liées

## License

Par contribution, vous acceptez que votre code soit publié sous la même licence que le projet.

## Questions ?

- Ouvrez une discussion sur GitHub
- Créez une issue pour les bugs
- Contactez l'équipe de maintenance

Merci de contribuer !
