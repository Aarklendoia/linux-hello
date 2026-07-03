# CI/CD Infrastructure

Documentation for the GitHub Actions CI/CD pipeline for Linux Hello.

## Workflows

### 1. build-debian.yml

**Triggered by:** Push on main, tags v*, PR, manual

Builds the Debian packages in a Debian Bookworm container.

Steps:

- Installation of build dependencies
- `dpkg-buildpackage` to create the packages
- Verification with `lintian`
- Upload of artifacts (.deb, .buildinfo, .changes packages)
- GitHub release creation if tag v*

Generated artifacts:

- `linux-hello_1.0.0-1_amd64.deb` (main package)
- `linux-hello-daemon_1.0.0-1_amd64.deb` (daemon)
- `linux-hello-gui_1.0.0-1_amd64.deb` (GUI interface)
- `linux-hello-tools_1.0.0-1_amd64.deb` (CLI tools)
- `libpam-linux-hello_1.0.0-1_amd64.deb` (PAM module)

### 2. test.yml

**Triggered by:** Push on main, PR, manual

Runs the unit tests and builds the release binaries.

Steps:

- Rust installation
- System dependency installation
- Cargo dependency cache (registry, git, target)
- `cargo test --all --release`
- `cargo clippy` (linting)
- `cargo build --all --release`

### 3. quality.yml

**Triggered by:** Push on main, PR, manual

Checks code quality (3 parallel jobs).

**Job 1 - format**:

- Checks `cargo fmt` (formatting)

**Job 2 - lint**:

- Checks `cargo clippy` with warnings as errors
- Installs build dependencies

**Job 3 - security**:

- Runs `cargo audit` to check for vulnerabilities

### 4. docs.yml

**Triggered by:** Push on docs/, *.md, manual

Generates Rust documentation with `cargo doc`.

Steps:

- Documentation build
- Copy to `public/` folder
- Markdown check (optional)
- Upload of artifacts

## Dependabot Configuration

Automatic dependency updates.

**Cargo**: Every Monday at 02:00 UTC

- Max 5 open PRs
- Label: `dependencies`
- Reviewer: `edouard`

**GitHub Actions**: Every Monday at 02:30 UTC

- Max 5 open PRs
- Label: `github-actions`
- Reviewer: `edouard`

Commit format: `chore: Update dependencies`

## Required GitHub Secrets

For automatic releases:

- `GITHUB_TOKEN` (automatic in GitHub Actions)

## Variants and conditions

### Conditionals by event

```yaml
# Triggers on push/PR
on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

# Additional triggers
  tags: [ 'v*' ]           # For releases
  workflow_dispatch:       # Manual

# Execution conditions
if: startsWith(github.ref, 'refs/tags/v')
```

## Running Workflows Locally

### With act (local runner)

```bash
# Install act
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | bash

# Run a workflow
act push

# Run a specific job
act -j build
```

## Artifacts Retention

- **Default**: 30 days
- **Configurable**: `retention-days`

Upload and download:

```bash
# Download in CI
actions/download-artifact@v4

# Upload from CI
actions/upload-artifact@v4
```

## Debugging

### Execution logs

Logs are available in the "Actions" tab on GitHub.

For each step:

- Execution time
- Full output
- Errors and warnings

### Re-run

"Re-run failed jobs" or "Re-run all jobs" button on the run page.

### Cache debugging

```bash
# List caches
gh actions-cache list --repo Aarklendoia/linux-hello

# Delete a cache
gh actions-cache delete <cache-key> --repo Aarklendoia/linux-hello
```

## Optimization

### Cache strategy

Cargo caches are organized by:

- Registry (`~/.cargo/registry`)
- Git dependencies (`~/.cargo/git`)
- Build artifacts (`target/`)

Cache keys include the hash of `Cargo.lock`.

### Parallel jobs

Independent jobs run in parallel:

- `format` (2 min)
- `lint` (5 min)
- `security` (3 min)

Total: ~5 min (instead of 10 min in series).

## Troubleshooting

### Failing builds

1. **Check the logs** in GitHub Actions
2. **Reproduce locally**: `dpkg-buildpackage -us -uc -b`
3. **Check dependencies**: `apt-get build-dep`
4. **Re-run** the workflow with debugging

### Invalid cache

Delete the cache:

```bash
gh actions-cache delete <key> --repo Aarklendoia/linux-hello
```

Or through the UI: Settings > Actions > Caches

### Network issues

Cargo timeouts may require:

- Increasing the `timeout` in seconds
- Adding alternative Cargo mirrors

## Best Practices

✅ **Do**:

- Test locally before pushing
- Keep workflows simple
- Use caches effectively
- Document CI/CD changes

❌ **Avoid**:

- Hardcoding secrets
- Massive artifact uploads
- Workflows that run too often
- Ignoring linting failures

## References

- [GitHub Actions Docs](https://docs.github.com/en/actions)
- [Dependabot Docs](https://docs.github.com/en/code-security/dependabot)
- [Act - Local Runner](https://github.com/nektos/act)
- [Debian dpkg-buildpackage](https://manpages.debian.org/bookworm/dpkg-dev/dpkg-buildpackage.1.en.html)
