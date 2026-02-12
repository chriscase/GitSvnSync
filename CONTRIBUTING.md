# Contributing to GitSvnSync

Thank you for your interest in contributing! This guide will help you get started.

## Development Setup

### Prerequisites

- Rust 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Node.js 20+ (for the React frontend)
- Docker & Docker Compose (for the test environment)
- SVN client (`apt install subversion` or `brew install svn`)

### Building

```bash
git clone https://github.com/chriscase/GitSvnSync.git
cd GitSvnSync
cargo build
```

### Running Tests

```bash
# Unit tests
cargo test

# Full test environment (E2E)
make test-env-up
make test-e2e
make test-env-down
```

### Frontend Development

```bash
cd web-ui
npm install
npm run dev    # Start Vite dev server with hot reload
```

## Project Structure

```
crates/
  core/       # Shared library — sync engine, SVN/Git clients, identity, conflicts, DB
  daemon/     # Daemon binary — entry point, scheduler, signals
  web/        # Web server — Axum, REST API, WebSocket
  cli/        # CLI tool — management commands
web-ui/       # React frontend — dashboard, conflict resolution, config
tests/        # E2E tests and Docker Compose environment
docs/         # Documentation
scripts/      # Install scripts, systemd unit
```

## Making Changes

1. **Fork** the repository
2. **Create a branch** from `main`: `git checkout -b feature/my-feature`
3. **Make your changes** with clear, focused commits
4. **Write tests** for new functionality
5. **Run the test suite**: `cargo test && cargo clippy && cargo fmt --check`
6. **Submit a pull request** with a clear description

## Code Style

- Follow standard Rust conventions (`cargo fmt`)
- Use `tracing` for logging (not `println!`)
- Use `thiserror` for error types
- Add doc comments to public APIs
- Keep functions focused and small

## Commit Messages

Follow conventional commits:
- `feat: add branch sync support`
- `fix: handle unicode filenames in SVN diff`
- `test: add E2E test for concurrent commits`
- `docs: update deployment guide`

## Reporting Issues

Use the GitHub issue templates:
- **Bug report**: Include steps to reproduce, expected vs actual behavior, logs
- **Feature request**: Describe the use case and proposed solution

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
