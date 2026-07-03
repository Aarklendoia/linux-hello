# Release Process

Guide for Linux Hello releases.

## Prerequisites

- Push access to the repository
- Access to GitHub secrets
- Development version ready

## Release Steps

### 1. Preparation

```bash
# Update versions
# - Cargo.toml
# - debian/changelog
# - VERSION file (if present)

# Verify that all tests pass
make test
make lint
make audit
```

### 2. Version commit

```bash
git add Cargo.toml debian/changelog VERSION
git commit -m "chore: Release version X.Y.Z"
```

### 3. Tag

```bash
git tag -a vX.Y.Z -m "Release version X.Y.Z"
# or for a release candidate
git tag -a vX.Y.Z-rc1 -m "Release candidate vX.Y.Z-rc1"
```

### 4. Push

```bash
git push origin main
git push origin vX.Y.Z
```

## Automatic CI/CD

Once the tag is pushed, GitHub Actions:

1. **Build Debian packages** - Creates the Debian packages
2. **Run tests** - Runs all the tests
3. **Run linting** - Checks code quality
4. **Create Release** - Creates a GitHub release with the artifacts

The packages are automatically:
- Built with dpkg-buildpackage
- Checked with lintian
- Uploaded as artifacts
- Added to the GitHub release

## Versioning

Linux Hello follows [Semantic Versioning](https://semver.org/lang/fr/):

- **MAJOR** - Incompatible changes
- **MINOR** - New features
- **PATCH** - Bug fixes

Example: `1.2.3`

## Changelog

The changelog follows the [Keep a Changelog](https://keepachangelog.com/lang/fr/) format:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- New feature

### Changed
- Changes

### Fixed
- Bug fixes

### Deprecated
- Deprecated features

### Removed
- Removed features

### Security
- Security fixes
```

## Debian Changelog

Update `debian/changelog`:

```bash
dch -i
# or
dch --distribution bookworm --urgency medium
```

## Pre-release Checks

- [ ] All tests pass
- [ ] Linting and formatting OK
- [ ] No vulnerabilities (cargo audit)
- [ ] Documentation up to date
- [ ] Changelog updated
- [ ] Version updated
- [ ] No critical TODO/FIXME

## Version Support

- **Latest** - Current version (main branch)
- **LTS** - Long-term support (dedicated branch)
- **EOL** - End of life (no support)

## Bug Reporting

Security bugs should be reported privately to the maintenance team.
