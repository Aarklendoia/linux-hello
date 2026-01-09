# Infrastructure CI/CD

Documentation de la chaîne CI/CD GitHub Actions pour Linux Hello.

## Workflows

### 1. build-debian.yml
**Déclenché par :** Push sur main/develop, tags v*, PR, manual

Construit les paquets Debian dans un conteneur Debian Bookworm.

Étapes :
- Installation des dépendances de build
- `dpkg-buildpackage` pour créer les paquets
- Vérification avec `lintian`
- Upload des artefacts (paquets .deb, .buildinfo, .changes)
- Création de release GitHub si tag v*

Artefacts générés :
- `linux-hello_1.0.0-1_amd64.deb` (paquet principal)
- `linux-hello-daemon_1.0.0-1_amd64.deb` (daemon)
- `linux-hello-gui_1.0.0-1_amd64.deb` (interface GUI)
- `linux-hello-tools_1.0.0-1_amd64.deb` (outils CLI)
- `libpam-linux-hello_1.0.0-1_amd64.deb` (module PAM)

### 2. test.yml
**Déclenché par :** Push sur main/develop, PR, manual

Lance les tests unitaires et buildera les binaires release.

Étapes :
- Installation de Rust
- Installation des dépendances système
- Cache des dépendances Cargo (registry, git, target)
- `cargo test --all --release`
- `cargo clippy` (linting)
- `cargo build --all --release`

### 3. quality.yml
**Déclenché par :** Push sur main/develop, PR, manual

Vérifie la qualité du code (3 jobs en parallèle).

**Job 1 - format** :
- Vérifie `cargo fmt` (formatage)

**Job 2 - lint** :
- Vérifie `cargo clippy` avec warnings comme erreurs
- Installe les dépendances de compilation

**Job 3 - security** :
- Lance `cargo audit` pour vérifier les vulnérabilités

### 4. docs.yml
**Déclenché par :** Push docs/, *.md, manual

Génère la documentation Rust avec `cargo doc`.

Étapes :
- Build de la documentation
- Copie vers dossier `public/`
- Vérification du markdown (optional)
- Upload des artefacts

## Configuration Dependabot

Mises à jour automatiques des dépendances.

**Cargo** : Chaque lundi à 02:00 UTC
- Max 5 PRs ouvertes
- Label: `dependencies`
- Révision: `edouard`

**GitHub Actions** : Chaque lundi à 02:30 UTC
- Max 5 PRs ouvertes
- Label: `github-actions`
- Révision: `edouard`

Format des commits : `chore: Update dependencies`

## Secrets GitHub nécessaires

Pour les releases automatiques :
- `GITHUB_TOKEN` (automatique dans GitHub Actions)

## Variantes et conditions

### Conditionnels par événement

```yaml
# Déclenche sur push/PR
on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main, develop ]

# Triggers additionnels
  tags: [ 'v*' ]           # Pour releases
  workflow_dispatch:       # Manuel

# Conditions d'exécution
if: startsWith(github.ref, 'refs/tags/v')
```

## Running Workflows Locally

### Avec act (local runner)

```bash
# Installer act
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | bash

# Lancer un workflow
act push

# Lancer un job spécifique
act -j build
```

## Artifacts Retention

- **Défaut** : 30 jours
- **Configurable** : `retention-days`

Upload et téléchargement :
```bash
# Download dans CI
actions/download-artifact@v4

# Upload depuis CI
actions/upload-artifact@v4
```

## Debugging

### Logs d'exécution

Les logs sont disponibles dans l'onglet "Actions" de GitHub.

Pour chaque step :
- Temps d'exécution
- Output complet
- Erreurs et warnings

### Re-run

Bouton "Re-run failed jobs" ou "Re-run all jobs" sur la page de run.

### Cache debugging

```bash
# Lister les caches
gh actions-cache list --repo Aarklendoia/linux-hello

# Supprimer un cache
gh actions-cache delete <cache-key> --repo Aarklendoia/linux-hello
```

## Optimization

### Cache stratégie

Les caches Cargo sont organisés par :
- Registry (`~/.cargo/registry`)
- Git dependencies (`~/.cargo/git`)
- Build artifacts (`target/`)

Clés de cache incluent le hash de `Cargo.lock`.

### Parallel jobs

Les jobs indépendants s'exécutent en parallèle :
- `format` (2 min)
- `lint` (5 min)
- `security` (3 min)

Total : ~5 min (au lieu de 10 min en série).

## Troubleshooting

### Builds qui échouent

1. **Vérifier les logs** dans GitHub Actions
2. **Reproduire localement** : `dpkg-buildpackage -us -uc -b`
3. **Vérifier les dépendances** : `apt-get build-dep`
4. **Re-run** le workflow avec debugging

### Cache invalid

Supprimer le cache :
```bash
gh actions-cache delete <key> --repo Aarklendoia/linux-hello
```

Ou à travers le UI : Settings > Actions > Caches

### Network issues

Les timeouts Cargo peuvent nécessiter :
- Augmenter `timeout` en secondes
- Ajouter mirrors Cargo alternatifs

## Bonnes pratiques

✅ **À faire**:
- Tester localement avant de push
- Garder les workflows simples
- Utiliser les caches effectivement
- Documenter les changements CI/CD

❌ **À éviter**:
- Hardcoder des secrets
- Uploads massifs d'artefacts
- Workflows qui s'exécutent trop souvent
- Ignorer les échecs de linting

## References

- [GitHub Actions Docs](https://docs.github.com/en/actions)
- [Dependabot Docs](https://docs.github.com/en/code-security/dependabot)
- [Act - Local Runner](https://github.com/nektos/act)
- [Debian dpkg-buildpackage](https://manpages.debian.org/bookworm/dpkg-dev/dpkg-buildpackage.1.en.html)
