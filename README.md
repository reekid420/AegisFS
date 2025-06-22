# AegisFS

A modern, feature-rich filesystem with advanced capabilities including journaling, snapshots, encryption, and tiered storage.

## Features

### Implemented
- Basic FUSE-based filesystem
- Device formatting with AegisFS
- Simple file operations (read/write)
- Basic directory operations

### Planned
- **Journaling & Ordered Writes** - Ensure data consistency
- **Block Checksums** - Detect and repair data corruption
- **Snapshots** - Point-in-time recovery with CoW metadata
- **Encryption** - Optional AES-GCM encryption
- **Compression** - LZ4/ZSTD compression with deduplication
- **Tiered Storage** - Intelligent data placement across storage tiers
- **Cross-platform** - Linux, Windows, and macOS support

## Project Status

🚀 **Phase 0 Completed** - Research & Foundations (See [dev-roadmap.md](dev-roadmap.md) for details)

### Phase 0 Completed Features

- [x] Basic repository setup and project structure
- [x] FUSE-based filesystem implementation
- [x] Device formatting tool with superblock, inode table, and block bitmap
- [x] Basic file operations (create, read, write)
- [x] Directory operations (create, list)
- [x] CLI tools for formatting and mounting
- [x] Unit and integration tests
- [x] Initial documentation

### Current Focus

- Phase 1: FUSE-Based User-Space Prototype (In Progress)
  - Core modules implementation
  - Volume & partition management
  - Enhanced CLI management tool
  - Testing & benchmarking

See [dev-roadmap.md](dev-roadmap.md) for detailed progress.

## Getting Started

### Prerequisites

- Rust (latest stable)
- Cargo
- FUSE (for development)
- Docker (for containerized development)

### Building from Source

```bash
git clone https://github.com/your-username/aegisfs.git
cd aegisfs
cargo build --release
```

### Formatting a Device

To format a device or file as an AegisFS filesystem:

```bash
# Format a device (replace /dev/sdX with your device)
cargo run --bin format -- /path/to/device 3

# Format a file (useful for testing)
truncate -s 3G test.img
cargo run --bin format -- test.img 3
```

### Mounting the Filesystem

```bash
# Create a mount point
mkdir /mnt/aegisfs

# Mount the filesystem
cargo run --release -- /path/to/device /mnt/aegisfs

# When done, unmount with:
fusermount -u /mnt/aegisfs
```

### Development Commands

```bash
# Run tests
cargo test

# Build with debug symbols
cargo build

# Build for release
cargo build --release
```

## Development

### Project Structure

- `fs-core/` - Core filesystem library (Rust + C/C++)
- `fs-app/` - Management application (CLI + GUI)
- `fs-kmod/` - Linux kernel module
- `docs/` - Documentation
- `ci/` - CI/CD configurations

## License

[Your License Here]

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details.

## Roadmap

See [dev-roadmap.md](dev-roadmap.md) for detailed development plans.
