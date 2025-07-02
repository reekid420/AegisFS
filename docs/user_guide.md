# AegisFS User Guide

Welcome to AegisFS! This guide will help you get started with using AegisFS for your storage needs.

## 📖 Table of Contents

- [What is AegisFS?](#what-is-aegisfs)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Basic Operations](#basic-operations)
- [Advanced Features](#advanced-features)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## 🚀 What is AegisFS?

AegisFS is a modern, feature-rich filesystem designed for safety, performance, and advanced data management. Key benefits include:

### ✨ Key Features

- **📁 Full POSIX Compatibility** - Works with all your existing applications
- **🔄 Instant Snapshots** - Create point-in-time backups instantly  
- **🔍 Data Integrity** - Built-in checksums prevent data corruption
- **⚡ High Performance** - Optimized for modern storage devices
- **🛡️ Data Safety** - Write-back caching with robust error handling
- **🔧 Easy Management** - Simple command-line interface

### 🎯 Perfect For

- **Development Environments** - Instant snapshots before major changes
- **Content Creation** - Version control for media and documents
- **Testing** - Isolated environments with quick rollback
- **Data Protection** - Regular snapshots for important data
- **Performance** - Fast I/O for demanding applications

## 💾 Installation

### System Requirements

- **Linux**: Ubuntu 20.04+, Fedora 35+, or compatible distribution
- **macOS**: macOS 12+ with macFUSE installed
- **Windows**: Windows 10+ with WSL2 (native support coming soon)
- **RAM**: Minimum 512MB available
- **Storage**: Any block device or file

### Quick Installation

#### Option 1: Download Pre-built Binaries

```bash
# Download the latest release
curl -L https://github.com/your-username/aegisfs/releases/latest/download/aegisfs-linux-x86_64.tar.gz | tar xz

# Move to system path
sudo mv aegisfs /usr/local/bin/

# Verify installation
aegisfs --version
```

#### Option 2: Build from Source

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt-get install fuse3 libfuse3-dev pkg-config build-essential

# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone and build
git clone https://github.com/your-username/aegisfs.git
cd aegisfs
./scripts/build-cross-platform.sh

# The binary will be at: fs-app/cli/target/release/aegisfs
```

#### Option 3: Package Managers

```bash
# Homebrew (macOS)
brew install aegisfs

# APT (Ubuntu/Debian) - Coming soon
sudo apt install aegisfs

# DNF (Fedora) - Coming soon  
sudo dnf install aegisfs
```

### Setup FUSE Permissions

```bash
# Add your user to the fuse group
sudo usermod -a -G fuse $USER

# Logout and login again for changes to take effect
# Or in a new shell:
newgrp fuse

# Verify FUSE is available
ls -la /dev/fuse
# Should show: crw-rw-rw- 1 root fuse /dev/fuse
```

## 🚀 Quick Start

### Create Your First Filesystem

```bash
# 1. Create a 1GB filesystem file
aegisfs format mydata.img --size 1

# 2. Create a mount point
mkdir ~/mydata

# 3. Mount the filesystem
aegisfs mount mydata.img ~/mydata

# 4. Use it like any directory!
echo "Hello AegisFS!" > ~/mydata/welcome.txt
cat ~/mydata/welcome.txt

# 5. Unmount when done
fusermount -u ~/mydata
```

### Your First Snapshot

```bash
# Mount your filesystem
aegisfs mount mydata.img ~/mydata

# Add some data
echo "Important data" > ~/mydata/important.txt
mkdir ~/mydata/documents

# Create a snapshot before making changes
aegisfs snapshot mydata.img create "before-changes"

# Make some changes
echo "Modified data" > ~/mydata/important.txt
rm -rf ~/mydata/documents

# Oops! Rollback to the snapshot
fusermount -u ~/mydata
aegisfs snapshot mydata.img rollback "before-changes"
aegisfs mount mydata.img ~/mydata

# Your data is restored!
cat ~/mydata/important.txt  # Shows "Important data"
ls ~/mydata/documents       # Directory exists again
```

## 📋 Basic Operations

### Filesystem Management

#### Creating Filesystems

```bash
# Create filesystem in a file (recommended for testing)
aegisfs format myfs.img --size 5

# Create with custom name
aegisfs format myfs.img --size 10 --volume-name "My Storage"

# Format a real device (⚠️ DESTROYS ALL DATA!)
sudo aegisfs format /dev/sdb --size 100 --force
```

**Size Guidelines:**
- Small projects: 1-5 GB
- Development work: 10-50 GB  
- Media storage: 100+ GB
- Production use: Size based on needs

#### Mounting Filesystems

```bash
# Basic mount
mkdir mountpoint
aegisfs mount myfs.img mountpoint

# Mount read-only
aegisfs mount myfs.img mountpoint --read-only

# Mount with debug output (for troubleshooting)
aegisfs mount myfs.img mountpoint --debug
```

#### Unmounting

```bash
# Unmount filesystem
fusermount -u mountpoint

# Force unmount if stuck
sudo umount -f mountpoint
```

### File Operations

Once mounted, AegisFS filesystems work exactly like regular directories:

```bash
# All standard operations work
touch file.txt
mkdir directory
cp /etc/hosts .
mv file.txt newname.txt
rm oldfile.txt
ln -s target.txt symlink.txt

# Check filesystem usage
df -h mountpoint

# View detailed file information
ls -la mountpoint/
```

### Snapshot Management

#### Creating Snapshots

```bash
# Create a snapshot
aegisfs snapshot myfs.img create "snapshot-name"

# Create with description
aegisfs snapshot myfs.img create "before-update" --description "Before system update"

# Create with automatic naming
aegisfs snapshot myfs.img create "backup-$(date +%Y%m%d)"
```

#### Listing Snapshots

```bash
# List all snapshots
aegisfs snapshot myfs.img list

# List with detailed information
aegisfs snapshot myfs.img list --verbose

# List in JSON format (for scripts)
aegisfs snapshot myfs.img list --json
```

#### Restoring from Snapshots

```bash
# Rollback to a snapshot (must be unmounted first)
fusermount -u mountpoint
aegisfs snapshot myfs.img rollback "snapshot-name"
aegisfs mount myfs.img mountpoint
```

#### Deleting Snapshots

```bash
# Delete a snapshot
aegisfs snapshot myfs.img delete "old-snapshot"

# Force delete without confirmation
aegisfs snapshot myfs.img delete "old-snapshot" --force
```

### Filesystem Health

#### Checking Integrity

```bash
# Basic integrity check
aegisfs scrub myfs.img

# Deep integrity check
aegisfs scrub myfs.img --deep

# Check and fix errors
aegisfs scrub myfs.img --fix

# Check with progress display
aegisfs scrub myfs.img --progress
```

## 🎯 Advanced Features

### Automated Snapshots

Create scripts for regular snapshots:

```bash
#!/bin/bash
# snapshot-backup.sh

FILESYSTEM="mydata.img"
SNAPSHOT_NAME="auto-$(date +%Y%m%d-%H%M%S)"

# Create snapshot
aegisfs snapshot "$FILESYSTEM" create "$SNAPSHOT_NAME"

# Keep only last 7 snapshots
SNAPSHOTS=$(aegisfs snapshot "$FILESYSTEM" list --json | jq -r '.[].name' | head -n -7)
for snapshot in $SNAPSHOTS; do
    aegisfs snapshot "$FILESYSTEM" delete "$snapshot" --force
done
```

Add to cron for regular backups:

```bash
# Run every 4 hours
0 */4 * * * /path/to/snapshot-backup.sh
```

### Performance Optimization

#### Filesystem Tuning

```bash
# For SSDs - no additional tuning needed
# AegisFS is optimized for modern storage

# For HDDs - consider larger files to reduce fragmentation
# Use sequential write patterns when possible
```

#### Mount Options

```bash
# For high-performance scenarios
aegisfs mount myfs.img mountpoint --cache-size 1G

# For low-memory systems  
aegisfs mount myfs.img mountpoint --cache-size 64M
```

### Integration with System Tools

#### Backup Integration

```bash
# Use with rsync
rsync -av --progress source/ /mnt/aegisfs/backup/

# Create snapshot after backup
aegisfs snapshot myfs.img create "rsync-$(date +%Y%m%d)"
```

#### System Integration

```bash
# Add to /etc/fstab for automatic mounting
echo "/path/to/myfs.img /mnt/mydata aegisfs defaults,user 0 0" | sudo tee -a /etc/fstab

# Create systemd service for automatic mounting
sudo systemctl enable aegisfs-mount@mydata.service
```

### Multi-User Setup

```bash
# Create shared filesystem
sudo aegisfs format /dev/shared-disk --size 100 --force

# Mount with proper permissions
sudo mkdir /shared/data
sudo aegisfs mount /dev/shared-disk /shared/data
sudo chown -R users:users /shared/data
sudo chmod 755 /shared/data
```

## 🛡️ Best Practices

### Data Safety

1. **Regular Snapshots**
   ```bash
   # Before major changes
   aegisfs snapshot myfs.img create "before-$(whoami)-$(date +%Y%m%d)"
   ```

2. **Verify Integrity**
   ```bash
   # Weekly integrity checks
   aegisfs scrub myfs.img --deep
   ```

3. **Monitor Space**
   ```bash
   # Check available space regularly
   df -h /mnt/aegisfs
   ```

### Performance Tips

1. **Size Filesystems Appropriately**
   - Start with expected usage + 20% overhead
   - Growing filesystems is easier than shrinking

2. **Use Appropriate Snapshot Retention**
   - Keep snapshots based on importance
   - Delete old snapshots to save space

3. **Mount Options**
   - Use `--read-only` for archival data
   - Consider cache size for your workload

### Workflow Integration

1. **Development**
   ```bash
   # Snapshot before major code changes
   aegisfs snapshot devfs.img create "pre-refactor"
   
   # Work on code...
   
   # If changes work well, create success snapshot
   aegisfs snapshot devfs.img create "post-refactor-success"
   ```

2. **Content Creation**
   ```bash
   # Version your creative projects
   aegisfs snapshot projects.img create "project-v1.0"
   aegisfs snapshot projects.img create "project-v1.1-draft"
   ```

3. **System Administration**
   ```bash
   # Before system updates
   aegisfs snapshot system-data.img create "pre-update-$(date +%Y%m%d)"
   ```

## 🐛 Troubleshooting

### Common Issues

#### Mount Fails with "Permission Denied"

```bash
# Check FUSE permissions
ls -la /dev/fuse

# Add user to fuse group
sudo usermod -a -G fuse $USER
newgrp fuse

# Verify group membership
groups
```

#### "Device or resource busy"

```bash
# Check what's using the mount point
sudo lsof +D /mnt/aegisfs

# Force unmount
sudo umount -f /mnt/aegisfs

# Or lazy unmount
sudo umount -l /mnt/aegisfs
```

#### Filesystem Appears Corrupted

```bash
# Run integrity check
aegisfs scrub myfs.img --deep

# Attempt repair
aegisfs scrub myfs.img --fix

# If severe, rollback to known good snapshot
aegisfs snapshot myfs.img rollback "known-good-snapshot"
```

#### Poor Performance

```bash
# Check system resources
htop

# Monitor filesystem I/O
sudo iotop

# Check for fragmentation (if using files)
du -sh myfs.img
ls -la myfs.img

# Consider recreating filesystem if severely fragmented
```

#### Out of Space

```bash
# Check usage
df -h /mnt/aegisfs

# Clean up snapshots
aegisfs snapshot myfs.img list
aegisfs snapshot myfs.img delete "old-snapshot"

# For file-based filesystems, can sometimes extend
# (Advanced users only)
```

### Getting Help

#### Debug Information

```bash
# Mount with debug output
aegisfs mount myfs.img mountpoint --debug

# Check system logs
journalctl -u aegisfs
dmesg | grep -i fuse

# Enable verbose logging
export RUST_LOG=debug
aegisfs mount myfs.img mountpoint
```

#### Reporting Issues

When reporting issues, include:

1. **System information**
   ```bash
   uname -a
   aegisfs --version
   cat /etc/os-release
   ```

2. **Error messages**
   - Complete error output
   - System log entries
   - Debug output if available

3. **Steps to reproduce**
   - Exact commands used
   - Expected vs actual behavior
   - Filesystem size and type

#### Community Support

- **GitHub Issues**: [Report bugs and feature requests](https://github.com/your-username/aegisfs/issues)
- **Discussions**: [Ask questions and share tips](https://github.com/your-username/aegisfs/discussions)
- **Documentation**: [Latest docs and guides](https://github.com/your-username/aegisfs/docs)

## 📚 Additional Resources

### Documentation

- [Architecture Guide](architecture.md) - Technical details
- [Development Guide](development.md) - Contributing to AegisFS
- [API Reference](api_reference.md) - Programming interface
- [Build Guide](BUILD.md) - Compilation instructions

### Examples and Tutorials

- [Example Scripts](../examples/) - Automation and integration
- [Performance Benchmarks](../benches/) - Performance testing
- [Test Cases](../fs-core/tests/) - Example usage patterns

---

**AegisFS** - Modern filesystem technology made simple. Start with basic operations and grow into advanced features as your needs evolve!