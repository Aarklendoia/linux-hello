# Release Process

Guide pour les releases de Linux Hello.

## Prérequis

- Accès push au repository
- Accès aux secrets GitHub
- Version de développement prête

## Étapes de release

### 1. Préparation

```bash
# Mettre à jour les versions
# - Cargo.toml
# - debian/changelog
# - VERSION file (si présent)

# Vérifier que tous les tests passent
make test
make lint
make audit
```

### 2. Commit de version

```bash
git add Cargo.toml debian/changelog VERSION
git commit -m "chore: Release version X.Y.Z"
```

### 3. Tag

```bash
git tag -a vX.Y.Z -m "Release version X.Y.Z"
# ou pour une release candidate
git tag -a vX.Y.Z-rc1 -m "Release candidate vX.Y.Z-rc1"
```

### 4. Push

```bash
git push origin main
git push origin vX.Y.Z
```

## CI/CD automatique

Une fois le tag pushé, GitHub Actions :

1. **Build Debian packages** - Crée les paquets Debian
2. **Run tests** - Lance tous les tests
3. **Run linting** - Vérifie la qualité du code
4. **Create Release** - Crée une release GitHub avec les artefacts

Les paquets sont automatiquement :
- Buildés avec dpkg-buildpackage
- Vérifiés avec lintian
- Uploadés comme artifacts
- Ajoutés à la release GitHub

## Versioning

Linux Hello suit le [Semantic Versioning](https://semver.org/lang/fr/) :

- **MAJOR** - Changements incompatibles
- **MINOR** - Nouvelles fonctionnalités
- **PATCH** - Corrections de bugs

Exemple : `1.2.3`

## Changelog

Le changelog suit le format [Keep a Changelog](https://keepachangelog.com/lang/fr/) :

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- Nouvelle fonctionnalité

### Changed
- Changements

### Fixed
- Corrections de bugs

### Deprecated
- Fonctionnalités dépréciées

### Removed
- Fonctionnalités supprimées

### Security
- Corrections de sécurité
```

## Debian Changelog

Mettre à jour `debian/changelog` :

```bash
dch -i
# ou
dch --distribution bookworm --urgency medium
```

## Vérifications avant release

- [ ] Tous les tests passent
- [ ] Linting et formatage OK
- [ ] Pas de vulnérabilités (cargo audit)
- [ ] Documentation à jour
- [ ] Changelog mis à jour
- [ ] Version mise à jour
- [ ] Pas de TODO/FIXME critiques

## Support des versions

- **Latest** - Version actuelle (main branch)
- **LTS** - Long-term support (branche dédiée)
- **EOL** - End of life (pas de support)

## Reporting de bugs

Les bugs de sécurité doivent être rapportés de manière privée à l'équipe de maintenance.
