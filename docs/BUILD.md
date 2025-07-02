# AegisFS Build Guide

This guide covers building AegisFS from source, cross-compilation, and advanced build configurations.

## 🚀 Quick Build

### Standard Build
```bash
# Clone repository
git clone https://github.com/your-username/aegisfs.git
cd aegisfs

# Build everything (recommended)
./scripts/build-cross-platform.sh

# Binaries will be available at:
# fs-app/cli/target/release/aegisfs
```

### Test Build
```bash
# Build and run all tests
./scripts/build-cross-platform.sh test
```

## 📋 Prerequisites

### System Requirements

- **Rust** 1.70+ (latest stable recommended)
- **Cargo** (included with Rust)
- **Git**
- **Platform-specific FUSE libraries** (see below)

### Platform Dependencies

#### Linux (Ubuntu/Debian)
```bash
sudo apt-get update
sudo apt-get install -y \
    fuse3 libfuse3-dev \
    pkg-config build-essential \
    libc6-dev curl git
```

#### Linux (Fedora/RHEL/CentOS)
```bash
sudo dnf install -y \
    fuse3-devel pkg-config \
    gcc gcc-c++ make \
    glibc-devel curl git
```

#### macOS
```bash
# Install Homebrew if needed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install macfuse pkg-config
```

#### Windows (WSL2)
```bash
# In WSL2 Ubuntu/Debian environment
sudo apt-get update
sudo apt-get install -y \
    fuse3 libfuse3-dev \
    pkg-config build-essential
```

### Rust Setup
```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Add required components
rustup component add rustfmt clippy llvm-tools-preview

# Install development tools (optional)
cargo install cargo-audit cargo-deny cargo-llvm-cov
```

## 🏗️ Build System Overview

### Project Structure
```
aegisfs/
├── fs-core/                    ← Core filesystem library
│   ├── Cargo.toml              ← Core dependencies & features
│   └── src/                    ← Rust source code
├── fs-app/
│   ├── cli/                    ← Unified CLI application
│   │   ├── Cargo.toml          ← CLI dependencies
│   │   └── src/                ← CLI source code
│   └── gui/                    ← GUI application (Tauri)
├── scripts/
│   ├── build-cross-platform.sh ← Main build script
│   └── ci-helpers.sh           ← CI/CD utilities
└── Dockerfile                  ← Container builds
```

### Build Script Features

The `./scripts/build-cross-platform.sh` script provides:

- **Platform Detection**: Automatically detects OS and configures builds
- **Dependency Checking**: Verifies required tools are installed
- **Feature Configuration**: Enables appropriate features per platform
- **Cross-compilation**: Supports multiple target architectures
- **Testing Integration**: Runs comprehensive test suites

## 🔧 Build Commands

### Basic Commands

```bash
# Build for current platform
./scripts/build-cross-platform.sh

# Build and test
./scripts/build-cross-platform.sh test

# Clean build artifacts
./scripts/build-cross-platform.sh clean

# Check dependencies only
./scripts/build-cross-platform.sh deps

# Show help
./scripts/build-cross-platform.sh help
```

### Manual Building

#### Core Library
```bash
cd fs-core

# Debug build
cargo build --all-features

# Release build
cargo build --release --all-features

# Build with specific features
cargo build --release --features "fuse,encryption,compression"
```

#### CLI Application
```bash
cd fs-app/cli

# Debug build
cargo build

# Release build
cargo build --release
```

### Feature Flags

The core library supports several feature flags:

```toml
[features]
default = []
fuse = ["dep:fuser", "dep:ctrlc", "dep:clap", "dep:env_logger"]  # FUSE filesystem support
encryption = ["aes-gcm", "hkdf"]                                 # AES-GCM encryption
compression = ["lz4_flex", "zstd"]                              # LZ4/ZSTD compression
std = []                                                        # Standard library support
```

#### Platform-Specific Features
- **Linux/Unix**: `fuse` feature enabled by default
- **macOS**: `fuse` feature with macFUSE support
- **Windows**: Limited feature set (no mounting yet)

## 🌍 Cross-Compilation

### Supported Targets

#### Tier 1 (Fully Supported)
- `x86_64-unknown-linux-gnu` (Linux 64-bit)
- `x86_64-apple-darwin` (macOS Intel)
- `aarch64-apple-darwin` (macOS Apple Silicon)

#### Tier 2 (Basic Support)
- `x86_64-pc-windows-msvc` (Windows 64-bit)
- `x86_64-unknown-freebsd` (FreeBSD 64-bit)

### Cross-Compilation Commands

```bash
# Install target
rustup target add x86_64-unknown-linux-gnu

# Cross-compile via build script
./scripts/build-cross-platform.sh cross x86_64-unknown-linux-gnu

# Manual cross-compilation
cd fs-core
cargo build --release --target x86_64-unknown-linux-gnu --features "fuse,encryption,compression"

cd ../fs-app/cli
cargo build --release --target x86_64-unknown-linux-gnu
```

### Cross-Compilation Examples

#### Linux to Windows
```bash
# Install Windows target
rustup target add x86_64-pc-windows-msvc

# Cross-compile (note: limited features on Windows)
./scripts/build-cross-platform.sh cross x86_64-pc-windows-msvc

# Binary location: fs-app/cli/target/x86_64-pc-windows-msvc/release/aegisfs.exe
```

#### Linux to macOS
```bash
# Install macOS targets
rustup target add x86_64-apple-darwin aarch64-apple-darwin

# Cross-compile for Intel Macs
./scripts/build-cross-platform.sh cross x86_64-apple-darwin

# Cross-compile for Apple Silicon
./scripts/build-cross-platform.sh cross aarch64-apple-darwin
```

## 🐳 Docker Builds

### Build Containers

```bash
# Development environment
docker build --target dev -t aegisfs:dev .

# CI testing environment
docker build --target ci -t aegisfs:ci .

# Minimal runtime environment
docker build --target runtime -t aegisfs:runtime .
```

### Container Features

#### Development Container (`dev` target)
- Full Rust toolchain with components
- FUSE development headers
- All development tools (cargo-audit, etc.)
- Interactive development environment

#### CI Container (`ci` target)
- Optimized for automated testing
- Pre-built dependencies for faster CI
- FUSE support for integration tests

#### Runtime Container (`runtime` target)
- Minimal Debian-based image
- Only runtime dependencies
- Production-ready binaries

### Using Docker for Development

```bash
# Interactive development
docker run -it --privileged \
    -v /dev/fuse:/dev/fuse \
    -v $(pwd):/workspace \
    aegisfs:dev

# Run tests in container
docker run --rm --privileged \
    -v /dev/fuse:/dev/fuse \
    aegisfs:ci
```

## 🧪 Testing Builds

### Test Categories

#### Unit Tests
```bash
# Run all unit tests
cd fs-core && cargo test --lib

# Run specific test
cd fs-core && cargo test --lib test_inode_allocation
```

#### Integration Tests
```bash
# Run integration tests (requires FUSE)
cd fs-core && cargo test --test persistence_test --test write_operations -- --test-threads=1

# Note: --test-threads=1 is CRITICAL for FUSE tests
```

#### Cross-Platform Tests
```bash
# Test on target platform
cargo test --target x86_64-unknown-linux-gnu --lib
```

### Performance Tests

```bash
# Run benchmarks
cd fs-core && cargo criterion

# Specific benchmark
cd fs-core && cargo criterion filesystem_ops
```

### Coverage Analysis

```bash
# Generate coverage report
cd fs-core
cargo install cargo-llvm-cov
cargo llvm-cov --all-features --workspace --lcov --output-path coverage.lcov

# HTML coverage report
cargo llvm-cov --html --all-features --workspace
open target/llvm-cov/html/index.html
```

## ⚡ Optimization

### Release Builds

#### Standard Release
```bash
cd fs-core && cargo build --release --all-features
cd fs-app/cli && cargo build --release
```

#### Optimized Release
```bash
# With link-time optimization
cd fs-core
RUSTFLAGS="-C lto=fat" cargo build --release --all-features

cd ../fs-app/cli
RUSTFLAGS="-C lto=fat" cargo build --release
```

#### Size-Optimized Release
```bash
# Minimize binary size
cd fs-app/cli
RUSTFLAGS="-C opt-level=z -C strip=symbols" cargo build --release
```

### Profile-Guided Optimization (Advanced)

```bash
# Step 1: Build with PGO instrumentation
cd fs-core
RUSTFLAGS="-C profile-generate=/tmp/pgo-data" \
    cargo build --release --all-features

# Step 2: Run representative workload
# (Run your typical filesystem operations)

# Step 3: Build with PGO optimization
RUSTFLAGS="-C profile-use=/tmp/pgo-data" \
    cargo build --release --all-features
```

## 🔍 Build Debugging

### Verbose Builds
```bash
# Show all compilation commands
cd fs-core && cargo build --verbose

# Show build timings
cd fs-core && cargo build --timings
```

### Dependency Analysis
```bash
# Show dependency tree
cd fs-core && cargo tree

# Check for duplicate dependencies
cd fs-core && cargo tree --duplicates

# Audit dependencies
cd fs-core && cargo audit
```

### Build Cache

```bash
# Enable incremental compilation
export CARGO_INCREMENTAL=1

# Set parallel build jobs
export CARGO_BUILD_JOBS=8

# Use shared target directory
export CARGO_TARGET_DIR=/tmp/aegisfs-target
```

## 🐛 Troubleshooting

### Common Build Issues

#### Missing FUSE Headers
```bash
# Symptoms: "fuse.h not found" or similar
# Solution: Install FUSE development packages

# Ubuntu/Debian
sudo apt-get install libfuse3-dev

# Fedora/RHEL
sudo dnf install fuse3-devel

# macOS
brew install macfuse
```

#### Linker Errors
```bash
# Symptoms: Linker errors during final build step
# Solution: Install build tools

# Ubuntu/Debian
sudo apt-get install build-essential

# Fedora/RHEL
sudo dnf groupinstall "Development Tools"
```

#### Cross-Compilation Failures
```bash
# Symptoms: Target not found errors
# Solution: Install target

rustup target add x86_64-unknown-linux-gnu
rustup target list --installed
```

#### Out of Memory During Build
```bash
# Reduce parallel jobs
export CARGO_BUILD_JOBS=2

# Use less optimization for debug builds
cd fs-core && cargo build --profile dev
```

### Platform-Specific Issues

#### macOS: macFUSE Not Found
```bash
# Install macFUSE
brew install macfuse

# May require system reboot for kernel extension
```

#### Windows: FUSE Not Supported
```bash
# Currently limited support on Windows
# Build without FUSE features
cd fs-core && cargo build --no-default-features
```

#### Linux: Permission Denied
```bash
# Add user to fuse group
sudo usermod -a -G fuse $USER
# Logout and login again
```

## 📦 Distribution

### Binary Packaging

#### Linux Packages
```bash
# Create .deb package (Ubuntu/Debian)
# (Packaging scripts in development)

# Create .rpm package (Fedora/RHEL)
# (Packaging scripts in development)
```

#### macOS
```bash
# Create .app bundle
# (Packaging scripts in development)

# Homebrew formula
# (In development)
```

#### Windows
```bash
# Create .msi installer
# (Packaging scripts in development)
```

### Checksums and Signatures

```bash
# Generate checksums
sha256sum fs-app/cli/target/release/aegisfs > aegisfs.sha256

# Verify checksum
sha256sum -c aegisfs.sha256
```

## 🔐 Security Considerations

### Reproducible Builds

```bash
# Use fixed Rust version
rustup default 1.75.0

# Set deterministic flags
export RUSTFLAGS="-C debuginfo=0 -C strip=symbols"

# Build with fixed timestamp
SOURCE_DATE_EPOCH=1672531200 cargo build --release
```

### Dependency Auditing

```bash
# Security audit
cd fs-core && cargo audit

# License checking
cd fs-core && cargo deny check
```

## 📈 Performance

### Build Performance

#### Faster Builds
```bash
# Use faster linker (Linux)
sudo apt install lld
export RUSTFLAGS="-C link-arg=-fuse-ld=lld"

# Use parallel frontend (unstable)
export RUSTFLAGS="-Z threads=8"
cargo +nightly build --release
```

#### Build Caching
```bash
# Use sccache for distributed builds
cargo install sccache
export RUSTC_WRAPPER=sccache
```

### Runtime Performance

Built binaries include optimizations for:
- **Fast I/O**: Async operations with Tokio
- **Memory Efficiency**: Minimal heap allocations
- **Cache Optimization**: Write-back caching with configurable intervals
- **SIMD**: Vectorized operations where applicable

---

This build guide covers all aspects of building AegisFS from source. For development workflows, see [development.md](development.md). 