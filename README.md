# AegisFS

A modern, feature-rich filesystem with advanced capabilities including journaling, snapshots, encryption, and tiered storage.

## 🎯 Project Status

**Phase 1: FUSE Implementation** - ✅ **COMPLETE** (December 2024 - January 2025)

### ✅ Implemented & Tested
- **✅ FUSE-based filesystem** - Fully functional userspace filesystem with complete file/directory operations
- **✅ Data persistence** - Real file data storage with write-back cache and robust error handling  
- **✅ Device formatting** - Format real block devices and files with AegisFS (tested on NVMe partitions)
- **✅ File operations** - Create, read, write, delete files with proper metadata and persistence
- **✅ Directory operations** - Create, list, navigate directory structures with full persistence
- **✅ Unified CLI** - Professional command-line interface with subcommands (72% size reduction from 4 binaries to 1)
- **✅ Snapshot framework** - Complete metadata management system with JSON persistence
- **✅ Block device abstraction** - Support for files and real devices (NVMe, SSD, etc.)
- **✅ Cross-platform builds** - Unix/Linux and Windows build systems
- **✅ Dual licensing** - MIT OR Apache-2.0 for maximum compatibility
- **✅ Production-ready structure** - Professional project layout matching enterprise standards

### 🚧 In Active Development
- **🚧 GUI Management Interface** - Tauri-based cross-platform GUI (early development)
- **🚧 Module Integration** - Connect journaling, checksums, and snapshots with live filesystem operations
- **🚧 Performance Optimization** - Benchmarking and optimization of critical paths

### 📋 Planned Features
- **Journaling & Ordered Writes** - Ensure data consistency and crash recovery
- **Block Checksums & Self-heal** - Detect and repair data corruption automatically  
- **Encryption** - Optional AES-GCM encryption for data at rest
- **Compression** - LZ4/ZSTD compression with deduplication
- **Tiered Storage** - Intelligent data placement across storage tiers
- **Kernel module** - High-performance kernel-space implementation

## 🚀 Quick Start

### Prerequisites

- **Rust** (latest stable)
- **FUSE** development headers:
  ```bash
  # Ubuntu/Debian
  sudo apt-get install fuse3 libfuse3-dev pkg-config
  
  # macOS  
  brew install macfuse pkg-config
  
  # Fedora/RHEL
  sudo dnf install fuse3-devel pkg-config
  ```

### Build AegisFS

```bash
git clone https://github.com/your-username/aegisfs.git
cd aegisfs

# Build everything (recommended method)
./scripts/build-cross-platform.sh
```

### Create and Use a Filesystem

```bash
# Create a 1GB test filesystem
truncate -s 1G test.img
./fs-app/cli/target/release/aegisfs format test.img --size 1

# Mount and use the filesystem
mkdir testmnt
./fs-app/cli/target/release/aegisfs mount test.img testmnt

# Use normally - create files, directories, etc.
echo "Hello AegisFS!" > testmnt/hello.txt
cat testmnt/hello.txt
ls -la testmnt/

# Unmount when done
fusermount -u testmnt
```

### Format Real Block Devices

```bash
# ⚠️ WARNING: This will destroy all data on the device!
# Replace /dev/sdX with your actual device
sudo ./fs-app/cli/target/release/aegisfs format /dev/sdX --size 100 --force

# Mount the formatted device
sudo mkdir /mnt/aegisfs
sudo ./fs-app/cli/target/release/aegisfs mount /dev/sdX /mnt/aegisfs
```

## 📖 Documentation

### Quick Reference
- **[Development Guide](docs/development.md)** - Setup, testing, and contribution workflow
- **[Architecture](docs/architecture.md)** - Technical design and module structure  
- **[Build Instructions](docs/BUILD.md)** - Detailed build and cross-compilation guide

### Command Reference

#### Core Commands
```bash
# Format a device/file
aegisfs format <device> --size <GB> [--force]

# Mount a filesystem  
aegisfs mount <device> <mountpoint>

# Create and manage snapshots
aegisfs snapshot <device> create <name>
aegisfs snapshot <device> list
aegisfs snapshot <device> delete <name>

# Check filesystem integrity
aegisfs scrub <device>
```

#### Getting Help
```bash
# General help
aegisfs --help

# Command-specific help
aegisfs format --help
aegisfs mount --help  
aegisfs snapshot --help
aegisfs scrub --help
```

## 🏗️ Architecture

### Project Structure
```
aegisfs/
├── fs-core/                    ← Core filesystem library
│   ├── src/                    ← FUSE implementation & modules
│   │   ├── modules/            ← Pluggable components
│   │   │   ├── journaling/     ← Transaction & crash recovery
│   │   │   ├── snapshot/       ← Snapshot management
│   │   │   └── checksums/      ← Data integrity verification
│   │   ├── blockdev/           ← Block device abstraction
│   │   ├── cache.rs            ← In-memory caching layer
│   │   ├── layout.rs           ← On-disk format & layout
│   │   └── lib.rs              ← Main FUSE filesystem
│   └── tests/                  ← Unit & integration tests
├── fs-app/                     ← Management applications
│   ├── cli/                    ← Unified command-line interface
│   └── gui/                    ← Tauri-based management GUI
├── docs/                       ← Documentation
├── scripts/                    ← Build & utility scripts
├── .github/workflows/          ← CI/CD automation
└── Dockerfile                  ← Development & testing containers
```

### Key Features

#### Modern Architecture
- **Modular Design** - Pluggable components for different features
- **FUSE-based** - Userspace implementation for safety and portability
- **Async I/O** - Tokio-based asynchronous operations for performance
- **Memory Safety** - Written in Rust with comprehensive error handling

#### Data Persistence
- **Write-back Cache** - 5-second flush interval with immediate sync on critical operations
- **Robust Error Handling** - 3x retry logic with graceful degradation
- **Inode Management** - Proper allocation tracking with bitmap persistence
- **Directory Persistence** - Parent-child relationships maintained on disk

#### Enterprise Ready
- **Dual Licensing** - MIT OR Apache-2.0 for maximum compatibility
- **Cross-platform** - Linux, macOS, Windows support
- **Professional CLI** - Intuitive subcommand interface
- **Comprehensive Testing** - Unit, integration, and persistence tests

## 🔧 Development

### Build System
```bash
# Build for current platform
./scripts/build-cross-platform.sh

# Run all tests  
./scripts/build-cross-platform.sh test

# Cross-compile for different targets
./scripts/build-cross-platform.sh cross x86_64-pc-windows-msvc

# Clean build artifacts
./scripts/build-cross-platform.sh clean
```

### Testing
```bash
# Unit tests
cd fs-core && cargo test --lib

# Integration tests (requires FUSE)
cd fs-core && cargo test --test persistence_test --test write_operations -- --test-threads=1

# All tests with coverage
cd fs-core && cargo llvm-cov --all-features --workspace --lcov --output-path coverage.lcov
```

### Docker Development
```bash
# Build development environment
docker build --target dev -t aegisfs:dev .

# Run interactive development
docker run -it --privileged -v /dev/fuse:/dev/fuse -v $(pwd):/workspace aegisfs:dev

# Run tests in container
docker build --target ci -t aegisfs:ci .
docker run --rm --privileged -v /dev/fuse:/dev/fuse aegisfs:ci
```

## 🧪 Testing & Verification

### Persistence Verification
The project includes comprehensive tests to verify data actually persists to disk:

```bash
# Run the critical persistence test
cd fs-core && cargo test --test persistence_test -- --test-threads=1
```

This test:
1. Formats a filesystem
2. Mounts it via FUSE
3. Writes test data
4. Unmounts the filesystem  
5. Checks raw device for the written data
6. Remounts and verifies data persistence

### Real Device Testing
AegisFS has been successfully tested on real NVMe partitions:

```bash
# Successfully tested on /dev/nvme0n1p6
sudo ./fs-app/cli/target/release/aegisfs format /dev/nvme0n1p6 --size 10 --force
sudo ./fs-app/cli/target/release/aegisfs mount /dev/nvme0n1p6 /mnt/test
```

## 📊 Performance Characteristics

### Current Implementation
- **Write-back Cache** - 5-second flush interval balances performance and data safety
- **Small File Optimization** - Files ≤4KB cached in memory for speed  
- **Async Operations** - Non-blocking I/O using Tokio thread pool
- **Error Recovery** - 3x retry logic for resilient operation

### Benchmarking
```bash
# Run performance benchmarks
cd fs-core && cargo criterion
```

## 🔒 Security & Reliability

### Data Safety
- **Write-back Cache** - Automatic periodic sync every 5 seconds
- **Manual Sync** - fsync() support for critical operations
- **Error Handling** - Comprehensive retry logic and graceful degradation
- **Crash Recovery** - Journaling framework (integration in progress)

### Security Features
- **Memory Safety** - Rust's ownership system prevents common vulnerabilities
- **Input Validation** - All user inputs validated and sanitized
- **Permission Checks** - POSIX permission enforcement
- **Audit Trail** - Comprehensive logging of filesystem operations

## 🤝 Contributing

We welcome contributions! Please see our [Development Guide](docs/development.md) for details on:

- Setting up the development environment
- Running tests and quality checks
- Submitting pull requests
- Code style and conventions

### Quick Contribution Workflow
```bash
# 1. Fork and clone the repository
git clone https://github.com/your-username/aegisfs.git
cd aegisfs

# 2. Create a feature branch
git checkout -b feature/your-feature

# 3. Make changes and test
./scripts/build-cross-platform.sh test

# 4. Submit a pull request
```

## 📄 License

This project is dual licensed under either of:

* **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* **MIT License** ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## 🗺️ Roadmap

See [dev-roadmap.md](dev-roadmap.md) for detailed development plans and timeline.

### Current Phase: Module Integration (Phase 1.5)
- Connect journaling system with filesystem operations
- Integrate checksums with block I/O operations
- Complete snapshot-filesystem integration

### Next Phase: GUI & Advanced Features (Phase 2)
- Complete Tauri-based management interface
- Implement encryption and compression modules
- Add tiered storage capabilities

---

**AegisFS** - Building the future of filesystem technology with safety, performance, and modularity.
