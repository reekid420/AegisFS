# Multi-stage Docker build for AegisFS development and testing

# Stage 1: Base development environment
FROM rust:1.75-slim as base

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libfuse3-dev \
    fuse3 \
    build-essential \
    libc6-dev \
    curl \
    git \
    sudo \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for development
RUN useradd -m -s /bin/bash developer && \
    usermod -a -G fuse developer && \
    echo 'developer ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers

# Install additional Rust components
RUN rustup component add rustfmt clippy llvm-tools-preview

# Install useful cargo tools
RUN cargo install cargo-audit cargo-deny cargo-llvm-cov cargo-criterion

WORKDIR /workspace
USER developer

# Stage 2: CI testing environment
FROM base as ci

# Copy the project
COPY --chown=developer:developer . .

# Pre-build dependencies for faster CI runs
RUN cd fs-core && cargo fetch

# Enable FUSE in container
RUN sudo modprobe fuse || true

# Default command for CI
CMD ["bash", "-c", "cd fs-core && cargo test --all-features"]

# Stage 3: Development environment with additional tools
FROM base as dev

# Install development tools
RUN cargo install cargo-watch cargo-expand

# Install debugging tools
RUN apt-get update && apt-get install -y \
    gdb \
    valgrind \
    strace \
    && rm -rf /var/lib/apt/lists/*

# Copy project files
COPY --chown=developer:developer . .

# Build the project
RUN cd fs-core && cargo build --release --all-features \
    && cd ../fs-app/cli && cargo build --release --all-features

# Expose any ports needed for development
EXPOSE 8080

# Default to bash for interactive development
CMD ["/bin/bash"]

# Stage 4: Minimal runtime environment
FROM debian:bookworm-slim as runtime

RUN apt-get update && apt-get install -y \
    fuse3 \
    && rm -rf /var/lib/apt/lists/*

# Create runtime user
RUN useradd -m -s /bin/bash aegisfs && \
    usermod -a -G fuse aegisfs

# Copy built binaries from development stage
COPY --from=dev --chown=aegisfs:aegisfs /workspace/fs-app/cli/target/release/aegisfs /usr/local/bin/aegisfs

USER aegisfs
WORKDIR /home/aegisfs

# Default to help for the unified CLI
CMD ["aegisfs", "--help"]
