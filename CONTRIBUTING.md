# Contributing to Linux Hello

Thank you for your interest in contributing to Linux Hello!

## Development Setup

See [DEVELOPMENT.md](.github/DEVELOPMENT.md) for detailed setup instructions.

## Code Style

### Rust Code
- Use `cargo fmt` for formatting
- Run `cargo clippy -- -D warnings` before submitting
- Write tests for new functionality
- Document public APIs with doc comments

### Shell Scripts
- Use `shellcheck` for validation
- Follow Debian policy guidelines
- Avoid bashisms in scripts that may run with `/bin/sh`

### Debian Packaging
- Follow Debian Policy Manual
- Use `quilt` for patch management
- Test package installation with `dpkg -i` and `apt-get install -yf`
- Update changelog entries with `dch -i`

## Submitting Changes

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests and linters:
   ```bash
   cargo test --lib
   cargo clippy -- -D warnings
   cargo fmt --check
   shellcheck bin/linux-hello-config
   ```
5. Commit with descriptive messages
6. Push to your fork and submit a pull request

## Pull Request Process

1. Ensure all CI checks pass
2. Update relevant documentation
3. Add changelog entry if applicable
4. Request review from maintainers
5. Address review feedback

## Issues

When reporting bugs:
- Provide reproduction steps
- Include system information (OS, version)
- Attach relevant logs if applicable

When suggesting features:
- Explain the use case
- Provide examples if possible
- Discuss implementation approach

## License

By contributing, you agree that your contributions will be licensed under the same license as the project.

## Questions?

Feel free to open an issue for questions or discussions!
