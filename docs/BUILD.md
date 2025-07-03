# AegisFS Cross-Platform Build Guide

This guide explains how to compile AegisFS for different operating systems and platforms.

## Quick Start

### Automatic Build (Recommended)

**Linux/macOS/Unix:**
```bash
# Make the script executable
chmod +x build-cross-platform.sh

# Build for your current platform
./build-cross-platform.sh

# Or cross-compile for Windows
./build-cross-platform.sh cross x86_64-pc-windows-msvc
```

**Windows:**
```batch
# Build for Windows
build-cross-platform.bat

# Or cross-compile for Linux
build-cross-platform.bat cross x86_64-unknown-linux-gnu
```

## Platform Support

| Platform | Status | Features Available |
|----------|--------|-------------------|
| Linux | ✅ Full Support | FUSE mounting, encryption, compression, all tools |
| macOS | ✅ Full Support | FUSE mounting, encryption, compression, all tools |
| Windows | 🟡 Partial Support | File operations, encryption, compression (no mounting yet) |
| FreeBSD | ✅ Full Support | FUSE mounting, encryption, compression, all tools |

## Manual Build Instructions

### Prerequisites

#### Common Requirements
- [Rust](https://rustup.rs/) (latest stable version)
- Git

#### Platform-Specific Requirements

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get update
sudo apt-get install libfuse3-dev pkg-config build-essential
```

**Linux (RHEL/Fedora):**
```bash
sudo yum install fuse3-devel pkgconfig gcc
```

**macOS:**
```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install macfuse pkg-config
```

**Windows:**
```batch
# Install Visual Studio Build Tools
# Download from: https://visualstudio.microsoft.com/visual-cpp-build-tools/

# Optional: Install WinFsp for future filesystem mounting support
# Download from: https://winfsp.dev/
```

### Build Commands

#### Build for Current Platform

**Linux/macOS/Unix:**
```bash
cd fs-core
cargo build --release --features "fuse,encryption,compression"
```

**Windows:**
```batch
cd fs-core
cargo build --release --features "encryption,compression"
```

#### Cross-Compilation

**From any platform to Windows:**
```bash
rustup target add x86_64-pc-windows-msvc
cd fs-core
cargo build --release --target x86_64-pc-windows-msvc --features "encryption,compression"
```

**From any platform to Linux:**
```bash
rustup target add x86_64-unknown-linux-gnu
cd fs-core
cargo build --release --target x86_64-unknown-linux-gnu --features "fuse,encryption,compression"
```

**From any platform to macOS:**
```bash
rustup target add x86_64-apple-darwin
cd fs-core
cargo build --release --target x86_64-apple-darwin --features "fuse,encryption,compression"
```

## Feature Flags

AegisFS uses feature flags to enable/disable functionality:

| Feature | Description | Default | Platforms |
|---------|-------------|---------|-----------|
| `fuse` | FUSE filesystem mounting | Auto-detected | Linux, macOS, FreeBSD |
| `winfsp` | Windows filesystem mounting | Auto-detected | Windows (future) |
| `encryption` | AES-GCM encryption support | Yes | All |
| `compression` | LZ4/ZSTD compression | Yes | All |

### Custom Feature Builds

```bash
# Minimal build (no encryption/compression)
cargo build --release --no-default-features

# Only encryption, no compression
cargo build --release --no-default-features --features "encryption"

# All features (where supported)
cargo build --release --features "fuse,encryption,compression"
```

## Testing

### Run Tests

**All platforms:**
```bash
cd fs-core
cargo test --features "encryption,compression"
```

**Linux/macOS (with FUSE tests):**
```bash
cd fs-core
cargo test --features "fuse,encryption,compression" -- --test-threads=1
```

### Integration Tests

**Note:** Integration tests require admin/root privileges for FUSE mounting:

```bash
# Linux/macOS
sudo -E cargo test --test persistence_test --features "fuse,encryption,compression" -- --test-threads=1

# Windows (run as Administrator)
cargo test --test write_operations --features "encryption,compression"
```

## Troubleshooting

### Common Issues

#### 1. FUSE Not Found (Linux/macOS)

**Error:** `pkg-config: command not found` or `fuse3 not found`

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install libfuse3-dev pkg-config

# macOS
brew install macfuse pkg-config

# Fedora/RHEL
sudo yum install fuse3-devel pkgconfig
```

#### 2. Permission Denied (Linux/macOS)

**Error:** `Permission denied` when mounting

**Solution:**
```bash
# Add user to fuse group
sudo usermod -a -G fuse $USER
# Then logout and login again

# Or enable user namespaces
echo 'user_allow_other' | sudo tee -a /etc/fuse.conf
```

#### 3. Visual Studio Build Tools Missing (Windows)

**Error:** `error: Microsoft C++ Build Tools`

**Solution:**
- Install Visual Studio Build Tools from: https://visualstudio.microsoft.com/visual-cpp-build-tools/
- Or install Visual Studio Community Edition

#### 4. Cross-compilation Linker Errors

**Error:** `linker cc not found` when cross-compiling

**Solution:**
```bash
# Install cross-compilation toolchain
# For Windows target from Linux:
sudo apt-get install gcc-mingw-w64

# For Linux target from macOS:
brew install FiloSottile/musl-cross/musl-cross
```

### Dependency Verification

Use the dependency checker:

```bash
# Linux/macOS
./build-cross-platform.sh deps

# Windows
build-cross-platform.bat deps
```

## Available Binaries

After building, you'll have **one unified binary**:

| Binary | Location | Description |
|--------|----------|-------------|
| `aegisfs` | `fs-app/cli/target/release/aegisfs` | Unified CLI (`format`, `mount`, `snapshot`, `scrub`, etc.) |

All functionality previously provided by `aegisfs-format`, `aegisfs-mount`, `aegisfs-snapshot`, and `aegisfs-scrub` is now available as subcommands of this single executable.

## Usage Examples

### Format and Mount (Linux/macOS)

```bash
# Create a test image
dd if=/dev/zero of=test.img bs=1M count=100

# Format with AegisFS
./fs-app/cli/target/release/aegisfs format test.img --size 100

# Create mount point
mkdir /tmp/aegisfs_mount

# Mount the filesystem
./fs-app/cli/target/release/aegisfs mount test.img /tmp/aegisfs_mount

# Use the filesystem
echo "Hello AegisFS!" > /tmp/aegisfs_mount/test.txt
cat /tmp/aegisfs_mount/test.txt

# Unmount
fusermount -u /tmp/aegisfs_mount
```

### File Operations (All Platforms)

```bash
# Create snapshots
./fs-app/cli/target/release/aegisfs snapshot test.img create "backup-$(date)"

# List snapshots
./fs-app/cli/target/release/aegisfs snapshot test.img list

# Check filesystem integrity
./fs-app/cli/target/release/aegisfs scrub test.img
```

## Development

### Setting up Development Environment

```bash
# Install development tools
rustup component add rustfmt clippy
cargo install cargo-audit cargo-deny

# Run development checks
cd fs-core
cargo fmt --all
cargo clippy --all-targets --all-features
cargo audit
```

### Contributing

1. Ensure your code compiles on all supported platforms
2. Run the full test suite
3. Follow the existing code style
4. Add tests for new functionality

For more detailed development information, see [docs/development.md](docs/development.md).

## License

AegisFS is dual-licensed under MIT OR Apache-2.0. 