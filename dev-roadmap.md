# Full Development Roadmap for "AegisFS" (All Bells & Whistles)

This end-to-end plan breaks the project into phases, each with milestones, deliverables, directory layout, tooling, and QA. Adjust timelines to fit your team size and velocity.

---

## Repository Root Layout

```
aegisfs/                     ← Project root
├── fs-core/                    ← Core filesystem library (Rust + C/C++)
│   ├── src/
│   │   ├── lib.rs              ← Rust FFI entry points
│   │   ├── modules/            ← Pluggable components
│   │   │   ├── journaling/
│   │   │   ├── snapshot/
│   │   │   ├── checksums/
│   │   │   ├── tiering/
│   │   │   ├── encryption/
│   │   │   ├── compression/
│   │   │   ├── dedupe/
│   │   │   ├── audit_logs/
│   │   │   ├── metrics/
│   │   │   └── vfs_layer/
│   │   ├── bindings/           ← C/C++ headers, Rust bindings
│   │   ├── blockdev/           ← Block device abstraction
│   │   ├── format/             ← Filesystem formatting
│   │   ├── cache.rs            ← Caching system
│   │   ├── error.rs            ← Error handling
│   │   └── layout.rs           ← Disk layout definitions
│   ├── include/                ← Public headers for kernel mode
│   ├── tests/                  ← Unit & integration tests
│   ├── benches/                ← Benchmark harnesses
│   ├── build.rs               ← Build script
│   ├── Cargo.toml             ← Rust package manifest
│   └── deny.toml              ← Dependency security config
├── fs-app/                     ← Management application
│   ├── cli/                    ← Command-line interface
│   │   ├── src/
│   │   │   ├── commands/       ← CLI subcommands
│   │   │   │   ├── format.rs
│   │   │   │   ├── mount.rs
│   │   │   │   ├── snapshot.rs
│   │   │   │   └── scrub.rs
│   │   │   └── main.rs         ← CLI entry point
│   │   └── Cargo.toml
│   ├── gui/                    ← Native GUI (Tauri + TypeScript)
│   │   ├── src/                ← Frontend source
│   │   │   └── assets/         ← Icons, images
│   │   ├── src-tauri/          ← Rust backend
│   │   │   ├── src/
│   │   │   ├── icons/          ← App icons
│   │   │   ├── capabilities/   ← Security permissions
│   │   │   ├── Cargo.toml
│   │   │   └── tauri.conf.json ← Tauri configuration
│   │   ├── package.json        ← Node.js dependencies
│   │   ├── vite.config.ts      ← Vite bundler config
│   │   └── tsconfig.json       ← TypeScript config
│   └── pkg/                    ← Build scripts, installers
├── fs-kmod/                    ← Linux kernel module prototype
│   └── src/
├── examples/                   ← Demo scripts & sample configs
├── docs/                       ← Design docs, API reference
│   ├── architecture.md
│   ├── BUILD.md
│   ├── development.md
│   └── DOCKER.md
├── scripts/                    ← Utility scripts (build, format, lint)
│   ├── build-cross-platform.sh ← Unix build script
│   ├── build-cross-platform.bat ← Windows build script
│   ├── check-env.sh           ← Environment validation
│   └── ci-helpers.sh          ← CI/CD utilities
├── dev-roadmap.md             ← Development roadmap (this file)
├── GUI_plan.md                ← GUI development plan
├── Dockerfile                 ← Dev & test container
├── LICENSE-MIT                ← MIT license
├── LICENSE-APACHE             ← Apache 2.0 license
└── README.md                  ← Project overview
```


---

## Phase 0: Research & Foundations (Completed) ✅

**Completion Date: June 21, 2025**

• **Setup & Tooling**  
  - [x] Initialize Git repo, CI pipelines, code style (rustfmt, clang-format)  
  - [x] Containerized dev environment (Docker + VSCode devcontainers)  

• **Architecture Design**  
  - [x] Finalize module interfaces, FFI boundaries, and VFS integration points  
  - [x] Write `architecture.md` with layered diagrams and data flows  

• **Minimal Proof-of-Concept**  
  - [x] FUSE "hello world" mount/unmount; stub VFS callbacks  
  - [x] Simple CLI to format a 3 GB partition and mount  
  - [x] Basic file operations (create/read/write)  
  - [x] Directory operations (create/list)  

**Deliverables**  
  - [x] Basic repo scaffolding  
  - [x] `architecture.md`  
  - [x] POC mount tool  
  - [x] Formatting tool with superblock, inode table, and block bitmap support  
  - [x] Basic documentation and usage examples  
  - [x] Unit and integration tests for core functionality  

---

## Phase 1: FUSE-Based User-Space Prototype (In Progress) 🚧
**Estimated Time: 6–8 Weeks**
**Current Status: Data Persistence Complete - Integration Testing (Week 6-7)**

### 1. **Data Persistence & FUSE Implementation** ✅
  - [x] **Critical Bug Discovery**: Found FUSE was only writing to memory
  - [x] **Architecture Refactoring**: Replaced in-memory VFS with disk-backed implementation
  - [x] **Disk Integration**: Connected FUSE layer to DiskFs for actual persistence
  - [x] **Data Block I/O**: Complete implementation of file data read/write to disk blocks
  - [x] **Inode Serialization**: Finished disk inode read/write operations
  - [x] **Cache Coherency**: Resolved borrowing issues in inode cache
  - [x] **Write-Back Cache**: Implemented hybrid caching with periodic flush (5s interval)
  - [x] **Inode Bitmap**: Proper inode allocation and management
  - [x] **Error Handling**: Retry logic with partial functionality on errors
  
  **Implementation Details:**
  - ✅ Dedicated thread pool for async disk operations
  - ✅ Hybrid write-back cache with configurable intervals
  - ✅ Proper inode bitmap for allocation tracking
  - ✅ Fast in-memory cache for small files (≤4KB)
  - ✅ Directory entries cached separately for speed
  - ✅ Error handling with 3x retry logic
  - ✅ Background flush task with periodic sync

### 2. Core Modules Implementation  
  - [🚧] **Journaling & Ordered Writes** (Framework exists, needs integration)
    - [x] Journal manager structure and transaction API
    - [ ] Integration with disk writes for atomic operations
    - [ ] Crash recovery testing and validation
  
  - [🚧] **Block Checksums + Self-heal** (Framework exists, needs integration)
    - [x] Checksum calculation and verification API
    - [ ] Integration with block device I/O layer
    - [ ] Background scrubbing implementation
  
  - [🚧] **Snapshot Engine (CoW Metadata)** (Framework exists, needs integration)
    - [x] Snapshot manager structure and CoW API
    - [ ] Integration with filesystem operations
    - [ ] Snapshot rollback functionality

### 3. Volume & Partition Management ✅ 
  - [x] **Block Device Abstraction**: File-backed and real device support
  - [x] **Filesystem Formatting**: Superblock, inode table, directory structures  
  - [x] **Real Device Support**: Successfully formatted real NVMe partition `/dev/nvme0n1p6`
  - [x] **Block Device Size Detection**: Fixed ioctl-based size detection for real block devices
  - [x] **Format Tool Issues**: Resolved all Arc ownership, size display, and validation issues
  - [x] **Device Mounting**: Reading formatted devices and initializing structures
  - [ ] Volume resize operations (grow/shrink)  
  - [ ] Multi-volume support
  - [ ] Volume status and health monitoring

### 4. CLI Management Tool ✅ 
  - [x] **Command structure and argument parsing**
  - [x] **Unified CLI Architecture**: Consolidated from 4 separate binaries to single `aegisfs` command
  - [x] **Professional User Experience**: Modern subcommand interface with shared global options
  - [x] **Core commands implemented**:
    - [x] `format` - Format a block device with AegisFS
    - [x] `mount` - Mount a formatted filesystem via FUSE
    - [x] `snapshot` - Full CLI with create/list/delete/rollback/stats, JSON persistence working
    - [x] `scrub` - Verify and repair filesystem integrity (framework implemented)
  - [🚧] **Additional commands**:
    - [ ] `resize` - Resize filesystem
  - [x] **Build System Integration**: Cross-platform scripts updated for unified CLI
  - [ ] Scheduler for automated tasks (snapshots, scrubs)
  - [ ] Configuration management

### 5. Testing & Validation  
  - [x] **Persistence Testing**: Created test to verify disk vs memory storage
  - [x] **Unit test coverage** for basic modules
  - [🚧] **Integration tests** for end-to-end operations
  - [ ] **Performance benchmarking**:
    - [ ] I/O throughput (sequential/random)
    - [ ] Metadata operations performance
    - [ ] FUSE vs native filesystem comparison
  - [ ] **Robustness testing**:
    - [ ] Power-loss simulation and recovery
    - [ ] Corruption detection and repair
    - [ ] Memory leak and performance regression tests

### Current Status & Key Achievements

**✅ Major Milestones Completed:**
- **Data Persistence**: Full disk I/O implementation with write-back cache, async thread pool, and 3x retry logic
- **FUSE Layer**: All core operations functional (mount, stat, create, read, write, mkdir, readdir)
- **CLI Unification**: Consolidated 4 separate binaries (11.4MB) into single unified CLI (3.2MB - 72% reduction)
- **Snapshot Framework**: Complete metadata management system with JSON persistence and CLI interface
- **Project Structure**: Repository layout matches roadmap specification with dual licensing (MIT OR Apache-2.0)
- **Real Device Support**: Successfully formatted and mounted real NVMe partition with proper size detection

**✅ Critical Bug Fixes:**
- **Layout Mismatch**: Fixed inconsistent inode_count calculations between format and mount operations
- **Persistence Issues**: Implemented deferred flush mechanism to avoid deadlocks and ensure directory entries persist to disk
- **Root Inode**: Corrected root inode number from 2 to 1, resolved runtime nesting panics

**✅ Technical Achievements:**
- **Cache Strategy**: Hybrid approach with small files (≤4KB) cached in memory, larger files written through to disk
- **Background Flush**: Automatic periodic sync with configurable intervals (5s default)
- **Directory Persistence**: Parent-child relationships properly maintained on disk
- **Cross-Platform Build**: Updated scripts for unified architecture on Windows and Unix systems

**🚧 In Progress:**
- Integration testing and performance optimization
- Module integration (journaling, checksums, snapshots) with filesystem operations
- Performance benchmarking and robustness testing

**📍 Phase 2 Started:**
- Tauri framework initialized for GUI development in parallel with Phase 1 completion

**Deliverables:**  
  ‣ [✅] **Fully persistent FUSE filesystem**
  ‣ [✅] **Professional Unified CLI** 
  ‣ [✅] **Production-Ready Project Structure**
  ‣ [ ] **Benchmark reports & CI integration**

---

## Phase 2: Management App & UI/UX (Started) 🚧
**Estimated Time: 4–6 Weeks**
**Current Status: Initial Setup and Configuration**

### 1. Native GUI Framework Selection - **Tauri** ✅ 
  **Chosen Framework: Rust + Tauri**
  - **Languages**: Rust (backend) + HTML/CSS/TS (frontend)
  - **Platforms**: Linux, Windows, macOS
  - **Single Binary**: ✅ Yes, very small binaries (~10-40MB)
  - **Embedded Assets**: ✅ All web assets embedded
  - **Why**: Excellent for system management apps, great performance, small binaries, active development
  
  **Progress:**
  - [x] **Tauri Project Initialized**: Basic project structure created in `fs-app/gui/`
  - [x] **Configuration Setup**: `tauri.conf.json` and capabilities configured
  - [x] **Build System**: TypeScript + Vite frontend toolchain configured
  - [🚧] **UI Development**: Initial HTML/CSS framework being implemented
  - [ ] **Backend Integration**: Connect to fs-core APIs
  - [ ] **Feature Implementation**: Tabs for Snapshots, Tiering, Settings

### 2. Integrate Core APIs  
  – REST/gRPC service layer from fs-core  
  – CLI & GUI share same API endpoints  

### 3. Features  
  - Slider controls (compression, encryption)  
  - Snapshot schedule editor & retention graph  
  - Real-time I/O charts (via metrics module)  

### 4. Packaging  
  - Rust/C++ cross-compilation for Linux, macOS, Windows  
  - Installers: `.deb`/`.rpm`/Homebrew/Win MSI  

Deliverables:  
  ‣ Polished GUI with all panels  
  ‣ Cross-platform install bundles  
  ‣ User manual in `docs/`  

---

## Phase 3: Advanced Services & Plugins (6–8 Weeks)

### 1. Optional Modules  
  - **AES-GCM Encryption** plugin  
  - **LZ4/ZSTD Compression & Dedupe**  
  - **Audit Logs (Merkle Trees)**  

### 2. Hybrid Tiering Engine  
  - 1-week hotness algorithm  
  - Migration CLI commands + UI controls  

### 3. Backup Integration  
  - rsync backend & rclone hook points  
  - Scheduled and on-demand backup tasks  

### 4. Plugin Framework  
  - Define plugin API (loadable `.so`/`.dll`)  
  - Sample plugin: "hello world" filter  

Deliverables:  
  ‣ 3 optional modules fully integrated  
  ‣ Hybrid tiering end-to-end demo  
  ‣ RSYNC backup workflow  

---

## Phase 4: Kernel-Mode Port (8–10 Weeks)

### 1. Kernel Module Scaffold  
  - Port FUSE callbacks to VFS hooks  
  - Expose ioctls for admin API  

### 2. Module Features  
  - Journaling, snapshots, checksums in kernel  
  - Online resize via block device interface  

### 3. Safety & Audit  
  - Kernel-side self-test, panic handlers, safe defaults  
  - Audit log sync with userspace  

### 4. Cross-Platform Drivers  
  - Prototype Windows Filter Driver  
  - macOS Kernel Extension (or DriverKit)  

Deliverables:  
  ‣ Linux kernel module in `fs-kmod/`  
  ‣ Windows/macOS stubs and WIP code  
  ‣ Kernel-mode performance benchmarks  

---

## Phase 5: Hardening, QA & Release (4–6 Weeks)

1. **Security Audit & Fuzzing**  
   - AFL/fuzzilli on FUSE callbacks  
   - Static analysis (Clippy, Coverity, SonarQube)  
2. **End-to-End Testing**  
   - Multi-OS VM tests (GitHub Actions matrix)  
   - Fault injection: power-loss, disk errors  
3. **Documentation & Tutorials**  
   - `docs/getting_started.md`, "Deep Dive" series  
   - Video walkthroughs & sample configs  
4. **Final Packaging & Versioning**  
   - SemVer release, CHANGELOG, GitHub Release  

Deliverables:  
   ‣ Security audit report  
   ‣ Stable 1.0 release packages  
   ‣ Complete docs & example repo  

---

## Phase 6: Post-Release & Ecosystem (Ongoing)

- **Community Plugins Marketplace**  
- **Cloud Sync Service** (P2P + S3/Azure)  
- **Enterprise Edition**: RBAC, multi-user quotas  
- **Performance Tuning**: GPU-accelerated crypto/checksums  
- **Long-term support branches**  

---

### Tooling & Infrastructure

- **Languages**: Rust (core), C/C++ (kernel/module)  
- **CI/CD**: GitHub Actions / Azure Pipelines  
- **Containerization**: Docker, Podman dev images  
- **Testing**: `criterion` for benchmarks, `tokio-test`  
- **Monitoring**: Prometheus exporters, Grafana dashboards  
- **Agile Board**: Jira or GitHub Projects  

---
