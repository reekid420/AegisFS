# Development Guide for AegisFS

This guide explains how to set up your development environment, contribute to AegisFS, and use the development tools effectively.

## 🚀 Quick Start

### One-Command Setup

```bash
# Clone and build
git clone https://github.com/your-username/aegisfs.git
cd aegisfs
./scripts/build-cross-platform.sh

# Run tests to verify everything works
./scripts/build-cross-platform.sh test
```

## 📋 Prerequisites

### System Requirements

- **Rust** (latest stable version)
- **Git**
- **FUSE** development headers (platform-specific)
- **Docker** (optional, for containerized development)

### Platform-Specific Dependencies

#### Ubuntu/Debian
```bash
sudo apt-get update
sudo apt-get install -y \
    fuse3 libfuse3-dev pkg-config \
    build-essential libc6-dev \
    curl git
```

#### macOS
```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install macfuse pkg-config
```

#### Fedora/RHEL/CentOS
```bash
sudo dnf install -y \
    fuse3-devel pkg-config \
    gcc gcc-c++ make \
    git curl
```

#### Windows (WSL2)
```bash
# In WSL2 Ubuntu
sudo apt-get update
sudo apt-get install -y \
    fuse3 libfuse3-dev pkg-config \
    build-essential
```

### FUSE Setup

```bash
# Enable FUSE module (Linux)
sudo modprobe fuse

# Add user to fuse group
sudo usermod -a -G fuse $USER

# Verify FUSE is available
ls -la /dev/fuse

# You may need to logout/login for group changes to take effect
```

### Rust Development Tools

```bash
# Install required Rust components
rustup component add rustfmt clippy llvm-tools-preview

# Install development tools
cargo install \
    cargo-audit \
    cargo-deny \
    cargo-llvm-cov \
    cargo-criterion \
    cargo-watch \
    cargo-expand
```

## 🏗️ Project Structure

```
aegisfs/
├── fs-core/                    ← Core filesystem library
│   ├── src/
│   │   ├── lib.rs              ← Main FUSE filesystem implementation
│   │   ├── modules/            ← Pluggable feature modules
│   │   │   ├── journaling/     ← Transaction system
│   │   │   ├── snapshot/       ← Snapshot management
│   │   │   └── checksums/      ← Data integrity
│   │   ├── blockdev/           ← Block device abstraction
│   │   ├── cache.rs            ← Caching layer
│   │   ├── format/             ← On-disk format
│   │   └── layout.rs           ← Filesystem layout
│   ├── tests/                  ← Unit & integration tests
│   │   ├── persistence_test.rs ← Critical data persistence test
│   │   └── write_operations.rs ← Write operation tests
│   ├── benches/                ← Performance benchmarks
│   └── Cargo.toml              ← Core library dependencies
├── fs-app/                     ← Applications
│   ├── cli/                    ← Unified command-line interface
│   │   ├── src/
│   │   │   ├── main.rs         ← CLI entry point
│   │   │   └── commands/       ← Subcommand implementations
│   │   │       ├── format.rs   ← Device formatting
│   │   │       ├── mount.rs    ← Filesystem mounting
│   │   │       ├── snapshot.rs ← Snapshot management
│   │   │       └── scrub.rs    ← Integrity checking
│   │   └── Cargo.toml          ← CLI dependencies
│   └── gui/                    ← Tauri-based GUI (in development)
├── docs/                       ← Documentation
├── scripts/                    ← Build and utility scripts
│   ├── build-cross-platform.sh ← Main build script
│   └── ci-helpers.sh           ← CI/CD helper scripts
└── Dockerfile                  ← Development containers
```

## 🔧 Development Workflow

### Building the Project

#### Standard Build (Recommended)
```bash
# Build everything with platform detection
./scripts/build-cross-platform.sh

# This automatically:
# 1. Detects your operating system
# 2. Checks for required dependencies
# 3. Builds fs-core with appropriate features
# 4. Builds the unified CLI application
# 5. Creates optimized release binaries
```

#### Manual Build (Advanced)
```bash
# Build core library
cd fs-core
cargo build --release --features "fuse,encryption,compression"

# Build CLI application  
cd ../fs-app/cli
cargo build --release

# Back to project root
cd ../..
```

#### Cross-Compilation
```bash
# Build for different targets
./scripts/build-cross-platform.sh cross x86_64-pc-windows-msvc
./scripts/build-cross-platform.sh cross x86_64-unknown-linux-gnu
./scripts/build-cross-platform.sh cross x86_64-apple-darwin
./scripts/build-cross-platform.sh cross aarch64-apple-darwin
```

### Testing Strategy

#### Unit Tests
```bash
# Run library unit tests
cd fs-core && cargo test --lib

# Run CLI unit tests
cd fs-app/cli && cargo test

# Run all unit tests via build script
./scripts/build-cross-platform.sh test
```

#### Integration Tests (Critical)
```bash
# Run FUSE integration tests (requires FUSE setup)
cd fs-core && cargo test --test persistence_test --test write_operations -- --test-threads=1

# The --test-threads=1 is CRITICAL for FUSE tests to avoid conflicts
```

#### Persistence Testing
The most important test verifies data actually persists to disk:

```bash
# Run the critical persistence test
cd fs-core && cargo test --test persistence_test -- --test-threads=1 --nocapture

# This test:
# 1. Formats a filesystem
# 2. Mounts it via FUSE  
# 3. Writes test data
# 4. Unmounts and checks raw device
# 5. Remounts and verifies persistence
```

#### Coverage Testing
```bash
# Generate coverage report
cd fs-core
cargo llvm-cov --all-features --workspace --lcov --output-path coverage.lcov

# View coverage in browser
cargo llvm-cov --html --all-features --workspace
open target/llvm-cov/html/index.html
```

### Code Quality Checks

#### Formatting
```bash
# Check formatting
cd fs-core && cargo fmt --all -- --check

# Fix formatting
cd fs-core && cargo fmt --all
```

#### Linting
```bash
# Run Clippy lints
cd fs-core && cargo clippy --all-targets --all-features -- -D warnings

# Run Clippy with fixes
cd fs-core && cargo clippy --all-targets --all-features --fix
```

#### Security Auditing
```bash
# Check for known vulnerabilities
cd fs-core && cargo audit

# Check licenses and dependencies
cd fs-core && cargo deny check
```

### Performance Testing

#### Benchmarks
```bash
# Run all benchmarks
cd fs-core && cargo criterion

# Run specific benchmark
cd fs-core && cargo criterion filesystem_ops

# View benchmark results
open target/criterion/report/index.html
```

#### Memory Profiling (Linux)
```bash
# Build with debug symbols
cd fs-core && cargo build --profile dev

# Run with Valgrind
valgrind --tool=memcheck --leak-check=full \
    ./target/debug/deps/persistence_test-* --test-threads=1
```

### Development Commands

#### File System Testing
```bash
# Create test filesystem
truncate -s 1G test.img
./fs-app/cli/target/release/aegisfs format test.img --size 1

# Mount for testing
mkdir testmnt
./fs-app/cli/target/release/aegisfs mount test.img testmnt

# Test operations
echo "Hello World" > testmnt/hello.txt
cat testmnt/hello.txt
ls -la testmnt/

# Unmount
fusermount -u testmnt
```

#### Real Device Testing (Advanced)
```bash
# ⚠️ WARNING: This destroys data on the device!
# Use a test partition or USB drive

# Format real device
sudo ./fs-app/cli/target/release/aegisfs format /dev/sdX --size 10 --force

# Mount real device
sudo mkdir /mnt/aegisfs-test
sudo ./fs-app/cli/target/release/aegisfs mount /dev/sdX /mnt/aegisfs-test

# Test and unmount
sudo fusermount -u /mnt/aegisfs-test
```

## 🐳 Docker Development

### Build Development Environment

```bash
# Build development container
docker build --target dev -t aegisfs:dev .

# Build CI testing container
docker build --target ci -t aegisfs:ci .

# Build runtime container
docker build --target runtime -t aegisfs:runtime .
```

### Interactive Development

```bash
# Run interactive development environment
docker run -it --privileged \
    -v /dev/fuse:/dev/fuse \
    -v $(pwd):/workspace \
    aegisfs:dev

# Inside container, run tests
cd /workspace
./scripts/build-cross-platform.sh test
```

### CI Testing in Docker

```bash
# Run full CI pipeline in container
docker run --rm --privileged \
    -v /dev/fuse:/dev/fuse \
    aegisfs:ci

# This runs all tests including FUSE integration tests
```

## 🧪 Testing Guidelines

### Filesystem-Specific Considerations

1. **Always use single-threaded execution** for FUSE tests:
   ```bash
   cargo test --test integration_test -- --test-threads=1
   ```

2. **Verify data persistence** in all filesystem tests:
   - Write data to mounted filesystem
   - Unmount completely
   - Remount and verify data is still there
   - Check raw device for data presence

3. **Use temporary directories** for all tests:
   ```rust
   use tempfile::TempDir;
   
   #[tokio::test]
   async fn test_something() {
       let temp_dir = TempDir::new().unwrap();
       let device_path = temp_dir.path().join("test.img");
       // ... rest of test
   }
   ```

4. **Test error conditions**:
   - Invalid device paths
   - Permission issues
   - Corrupted filesystems
   - Out of space scenarios

### Coverage Requirements

- **Unit Tests**: Aim for >80% line coverage
- **Integration Tests**: Cover all major user workflows
- **Error Path Testing**: Test failure scenarios
- **Performance Tests**: Benchmark critical operations

## 🔍 Debugging

### Local Debugging

```bash
# Enable debug logging
export RUST_LOG=debug
export RUST_BACKTRACE=1

# Run with logging
./fs-app/cli/target/debug/aegisfs mount device.img mountpoint
```

### FUSE Debugging

```bash
# Mount with FUSE debug output
./fs-app/cli/target/debug/aegisfs mount -d device.img mountpoint

# This shows all FUSE operations in real-time
```

### GDB Debugging

```bash
# Build with debug symbols
cd fs-core && cargo build --all-features

# Debug with GDB
gdb ./target/debug/deps/some_test
(gdb) run --test-threads=1
```

### Advanced Debugging Tools

```bash
# Analyze with strace (Linux)
strace -f ./fs-app/cli/target/debug/aegisfs mount device.img mountpoint

# Memory debugging with AddressSanitizer (nightly Rust)
cargo +nightly run -Z sanitizer=address

# Run MIRI for undefined behavior detection
cargo +nightly miri test --lib
```

## 📝 Contributing Guidelines

### Code Style

- **Use `cargo fmt`** for all formatting
- **Follow Rust naming conventions**
- **Add documentation** to public APIs with `///` comments
- **Fix all Clippy warnings** before submitting
- **Include unit tests** for new functionality

### Git Workflow

```bash
# 1. Fork the repository on GitHub
# 2. Clone your fork
git clone https://github.com/YOUR_USERNAME/aegisfs.git
cd aegisfs

# 3. Create feature branch from develop
git checkout -b feature/your-feature-name

# 4. Make changes and commit
git add .
git commit -m "Add: your feature description"

# 5. Run full test suite
./scripts/build-cross-platform.sh test

# 6. Push and create pull request
git push origin feature/your-feature-name
```

### Pull Request Checklist

Before submitting a PR, ensure:

- [ ] **All tests pass** locally
- [ ] **Code is formatted** (`cargo fmt`)
- [ ] **No Clippy warnings** (`cargo clippy`)
- [ ] **Security audit passes** (`cargo audit`)
- [ ] **Documentation updated** if needed
- [ ] **Integration tests pass** with `--test-threads=1`
- [ ] **Performance impact** considered and tested
- [ ] **Breaking changes** documented

### Review Process

1. **Automated CI** runs on all PRs
2. **Code review** by maintainers
3. **Testing verification** including manual testing if needed
4. **Documentation review** for user-facing changes
5. **Merge** after all checks pass

## 🐛 Troubleshooting

### Common Issues

#### FUSE Not Available
```bash
# Symptoms: "FUSE not available" errors
sudo modprobe fuse
ls -la /dev/fuse
# Should show: crw-rw-rw- 1 root fuse /dev/fuse
```

#### Permission Denied on Mount
```bash
# Symptoms: Permission denied when mounting
sudo usermod -a -G fuse $USER
# Then logout and login again
```

#### Tests Hanging
```bash
# Symptoms: Integration tests hang or fail
# Solution: Use --test-threads=1 for FUSE tests
cargo test --test persistence_test -- --test-threads=1
```

#### Build Failures
```bash
# Symptoms: Compilation errors
# Solution: Clean and rebuild
cd fs-core && cargo clean
./scripts/build-cross-platform.sh
```

#### Docker Issues
```bash
# Symptoms: FUSE not working in Docker
# Solution: Run with --privileged and mount /dev/fuse
docker run --privileged -v /dev/fuse:/dev/fuse aegisfs:dev
```

### Getting Help

1. **Check existing issues** on GitHub
2. **Review CI logs** for detailed error information
3. **Use debug logging** with `RUST_LOG=debug`
4. **Run tests individually** to isolate problems
5. **Ask questions** in GitHub Discussions

### Performance Issues

#### Slow Builds
```bash
# Use build cache
export CARGO_INCREMENTAL=1

# Parallel builds
export CARGO_BUILD_JOBS=8

# Use faster linker
cargo install lld
```

#### Slow Tests
```bash
# Run only fast tests
cargo test --lib

# Skip integration tests during development
cargo test --lib --bins
```

## 📊 Metrics and Monitoring

### Performance Monitoring

```bash
# Benchmark filesystem operations
cd fs-core && cargo criterion

# Profile memory usage
valgrind --tool=massif ./target/release/aegisfs format test.img --size 1

# Monitor I/O patterns
sudo iotop -p $(pgrep aegisfs)
```

### Development Metrics

```bash
# Lines of code
find . -name "*.rs" | xargs wc -l

# Test coverage
cd fs-core && cargo llvm-cov --summary-only

# Dependency tree
cd fs-core && cargo tree
```

---

This development guide provides everything you need to contribute effectively to AegisFS. The project welcomes contributions and follows standard Rust community practices for code quality and testing. 