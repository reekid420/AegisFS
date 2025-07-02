# AegisFS Docker Guide

This document explains how to build and run the official AegisFS Docker images for development, testing and production.

## Image Overview

The `Dockerfile` in the repository is a **multi-stage build** providing three key stages:

1. **base** – minimal Rust + system dependencies (foundation for the other stages)
2. **ci** – includes Rust tooling and a full checkout; optimised for running the test-suite inside CI/CD
3. **dev** – everything from `ci` plus extra developer conveniences such as `cargo-watch`, `gdb`, `valgrind` …
4. **runtime** – stripped, non-root image containing only the final `aegisfs` binary and the FUSE user-space library (ideal for production)

> Note The default (final) stage is `runtime`. If you need a different stage pass `--target <stage>` to the build command (see examples below). The CI workflow already does this automatically.

## Prerequisites

* Docker **20.10** or newer
* The host kernel **must have FUSE enabled**. On Linux this is usually already the case (`lsmod | grep fuse`).
* To run containers that access `/dev/fuse` you either need the **`--privileged`** flag *or* add the specific device via `--device=/dev/fuse` *and* the `SYS_ADMIN` capability.  The examples below use the simpler `--privileged` approach.

## Building the Image

```bash
# Build the full image (runtime stage) – fast because of BuildKit cache
docker build -t aegisfs:latest .

# Build the CI stage (includes Rust, cargo etc.)
docker build --target ci -t aegisfs:ci .

# Build the development stage
docker build --target dev -t aegisfs:dev .
```

### Build with Buildx (multi-platform)

The GitHub workflow already builds `linux/amd64` and `linux/arm64` images.  To reproduce locally:

```bash
docker buildx create --use --name aegisfs

docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --push \
  -t your-dockerhub-username/aegisfs:latest \
  .
```

## Running the Container

### Production / Runtime image

```bash
docker run -it --rm --privileged \
  -v /dev/fuse:/dev/fuse \
  -v $(pwd)/test.img:/data/aegisfs.img \
  aegisfs:latest \
  aegisfs format /data/aegisfs.img --size 1
```

### Test-suite (CI stage)

```bash
docker run --rm --privileged \
  -v /dev/fuse:/dev/fuse \
  aegisfs:ci bash -c "cd fs-core && cargo test --all-features"
```

### Interactive Development Environment

```bash
docker run --rm -it --privileged \
  -v /dev/fuse:/dev/fuse \
  -v $(pwd):/workspace \
  aegisfs:dev
```

Inside the container you are logged-in as the `developer` user with the full Rust tool-chain available.

## Common Pitfalls

1. **"fuse: device not found"** – ensure the host has the FUSE kernel module loaded (`sudo modprobe fuse`).
2. **Permission errors** – the container runs as an unprivileged user that belongs to the `fuse` group; make sure you keep the `--privileged` flag or grant the required capabilities.
3. **SELinux / AppArmor** – tighten security profiles may block FUSE.  Temporarily disable the profile for quick testing or add explicit exceptions.

---

For further questions please refer to [docs/BUILD.md](BUILD.md) or open an issue in the tracker.