# Development Guide for AegisFS

This guide explains how to set up your development environment and use the CI/CD tools for AegisFS.

## Quick Start

### Prerequisites

- **Rust** (latest stable version)
- **Git**
- **Docker** (optional, for containerized development)
- **FUSE** development headers (platform-specific)

### One-Command Setup

```bash
# Install dependencies and run full CI pipeline
./scripts/ci-helpers.sh install-deps
./scripts/ci-helpers.sh full-ci
```

## Development Environment Setup

### 1. Install System Dependencies

#### Ubuntu/Debian
```bash
sudo apt-get update
sudo apt-get install -y fuse3 libfuse3-dev pkg-config build-essential
```

#### macOS
```bash
brew install macfuse pkg-config
```

#### Fedora/RHEL
```bash
sudo yum install -y fuse3-devel pkgconfig gcc
```

### 2. Setup FUSE
```bash
# Enable FUSE module
sudo modprobe fuse

# Add user to fuse group (logout/login required)
sudo usermod -a -G fuse $USER

# Or use the helper script
./scripts/ci-helpers.sh setup-fuse
```

### 3. Install Rust Tools
```bash
# Install required components
rustup component add rustfmt clippy llvm-tools-preview

# Install development tools
cargo install cargo-audit cargo-deny cargo-llvm-cov cargo-criterion cargo-watch
```

## Development Workflow

### Code Quality Checks

```bash
# Format code
cargo fmt --all
# or
./scripts/ci-helpers.sh format

# Run linting
cargo clippy --all-targets --all-features -- -D warnings
# or
./scripts/ci-helpers.sh lint

# Security audit
cargo audit && cargo deny check
# or
./scripts/ci-helpers.sh audit
```

### Testing

```bash
# Unit tests
cargo test --lib

# Integration tests (requires FUSE)
cargo test --test persistence_test --test write_operations -- --test-threads=1
# or
./scripts/ci-helpers.sh test-integration

# Tests with coverage
cargo llvm-cov --all-features --workspace --lcov --output-path coverage.lcov
# or
./scripts/ci-helpers.sh test-coverage

# All tests
cargo test --all-features
```

### Performance Testing

```bash
# Run benchmarks
cd fs-core && cargo criterion
# or
./scripts/ci-helpers.sh benchmarks

# Memory profiling (Linux only)
valgrind --tool=memcheck --leak-check=full ./target/release/aegisfs-format --help
```

### Building

```bash
# Debug build
cargo build --all-features

# Release build
cargo build --release --all-features
# or
./scripts/ci-helpers.sh build

# Cross-compilation
cargo build --release --target x86_64-unknown-linux-musl
# or
./scripts/ci-helpers.sh build x86_64-unknown-linux-musl
```

## Docker Development

### Build Docker Images

```bash
# Development environment
docker build --target dev -t aegisfs:dev .
# or
./scripts/ci-helpers.sh docker-build dev aegisfs:dev

# CI testing environment
docker build --target ci -t aegisfs:ci .

# Runtime environment
docker build --target runtime -t aegisfs:runtime .
```

### Run in Docker

```bash
# Interactive development
docker run -it --privileged -v /dev/fuse:/dev/fuse -v $(pwd):/workspace aegisfs:dev

# Run tests in container
docker run --rm --privileged -v /dev/fuse:/dev/fuse aegisfs:ci cargo test --lib
# or
./scripts/ci-helpers.sh docker-test aegisfs:ci
```

## CI/CD Pipeline

### Local CI Testing

Run the complete CI pipeline locally before pushing:

```bash
./scripts/ci-helpers.sh full-ci
```

This runs:
1. Code formatting check
2. Linting and documentation check  
3. Security audit
4. Unit tests with coverage
5. Integration tests
6. Release build

### GitHub Actions Workflows

The project includes several automated workflows:

#### Main CI Pipeline (`.github/workflows/ci.yml`)
- **Triggers**: Push/PR to main/develop
- **Jobs**:
  - Quick checks (format, clippy, docs)
  - Security audit
  - Unit tests with coverage
  - Integration tests (with FUSE)
  - Cross-platform builds
  - Performance benchmarks (PR only)
  - Docker build and test
  - Memory safety checks (MIRI)
  - Release validation

#### Release Pipeline (`.github/workflows/release.yml`)
- **Triggers**: Git tags (`v*.*.*`)
- **Features**:
  - Cross-platform binary builds
  - Docker image publishing
  - Release notes generation
  - Checksum generation

#### Performance Monitoring (`.github/workflows/performance.yml`)
- **Triggers**: Daily schedule, main branch pushes, PRs affecting performance-critical paths
- **Features**:
  - Performance benchmarks
  - Memory profiling
  - Performance regression detection
  - Automatic PR comments with results

### CI Configuration Files

- `deny.toml` - Dependency security and license checking
- `Dockerfile` - Multi-stage Docker builds
- `.github/pull_request_template.md` - PR template for consistency

## Testing Guidelines

### Filesystem-Specific Testing

When testing filesystem operations:

1. **Always use single-threaded tests** for FUSE operations:
   ```bash
   cargo test --test integration_test -- --test-threads=1
   ```

2. **Verify data persistence**:
   - Write data to mounted filesystem
   - Unmount and remount
   - Verify data is still there

3. **Test error conditions**:
   - Invalid device paths
   - Permission issues
   - Corrupted filesystems
   - Out of space scenarios

4. **Use temporary directories** for all tests:
   ```rust
   let temp_dir = tempfile::TempDir::new().unwrap();
   let mount_path = temp_dir.path().join("mount");
   ```

### Coverage Requirements

- **Unit tests**: Aim for >80% coverage
- **Integration tests**: Cover all major user workflows
- **Performance tests**: Benchmark critical paths
- **Security tests**: Fuzz input validation

## Debugging

### Local Debugging

```bash
# Enable debug logging
export RUST_LOG=debug

# Run with backtrace
export RUST_BACKTRACE=1

# Debug build with symbols
cargo build --all-features

# Use debugger
gdb ./target/debug/aegisfs-mount
```

### FUSE Debugging

```bash
# Mount with debug output
./target/debug/aegisfs-mount -d device.img /mnt/point

# Monitor FUSE operations
fusermount -u /mnt/point  # unmount
mount.fuse device.img /mnt/point -o debug
```

### Memory Debugging

```bash
# Valgrind memory check
valgrind --tool=memcheck --leak-check=full ./target/release/aegisfs-format

# Address sanitizer (nightly Rust)
cargo +nightly run -Z sanitizer=address
```

## Best Practices

### Code Style
- Use `cargo fmt` for formatting
- Follow Rust naming conventions
- Add documentation to public APIs
- Use `cargo clippy` and fix all warnings

### Testing
- Write tests for new features
- Test error conditions
- Use property-based testing for complex logic
- Mock external dependencies

### Performance
- Profile before optimizing
- Use benchmarks to track regressions
- Consider memory allocations in hot paths
- Test with realistic data sizes

### Security
- Validate all inputs
- Use safe Rust patterns
- Audit cryptographic code carefully
- Test permission and access controls

## Troubleshooting

### Common Issues

1. **FUSE not available**:
   ```bash
   sudo modprobe fuse
   ls -la /dev/fuse
   ```

2. **Permission denied on mount**:
   ```bash
   sudo usermod -a -G fuse $USER
   # Logout and login again
   ```

3. **Build failures**:
   ```bash
   cargo clean
   ./scripts/ci-helpers.sh install-deps
   ```

4. **Test failures in CI**:
   - Check FUSE setup in CI environment
   - Ensure single-threaded execution for integration tests
   - Verify temporary directory cleanup

### Getting Help

- Check existing issues on GitHub
- Run `./scripts/ci-helpers.sh help` for tool usage
- Review CI logs for detailed error information
- Use `RUST_LOG=debug` for verbose output

## Contributing

1. **Fork the repository**
2. **Create a feature branch** from `develop`
3. **Make your changes** following the style guide
4. **Run local CI**: `./scripts/ci-helpers.sh full-ci`
5. **Write/update tests** for your changes
6. **Update documentation** if needed
7. **Submit a pull request** using the PR template

### PR Checklist

Before submitting a PR, ensure:
- [ ] All tests pass locally
- [ ] Code is formatted and linted
- [ ] Security audit passes
- [ ] Documentation updated
- [ ] Breaking changes documented
- [ ] Performance impact considered

The automated CI will run the same checks and provide feedback on your PR. 