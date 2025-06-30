# Dockerfile for AegisFS background agents development environment
FROM rust:1.75-slim

# Install system dependencies for AegisFS development
RUN apt-get update && apt-get install -y \
    pkg-config \
    libfuse3-dev \
    fuse3 \
    build-essential \
    libc6-dev \
    curl \
    git \
    sudo \
    gdb \
    valgrind \
    strace \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for development (following Cursor best practices)
RUN useradd -m -s /bin/bash ubuntu && \
    usermod -a -G fuse ubuntu && \
    echo 'ubuntu ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers

# Install Rust components
RUN rustup component add rustfmt clippy llvm-tools-preview

# Install useful cargo tools for development
RUN cargo install \
    cargo-audit \
    cargo-deny \
    cargo-llvm-cov \
    cargo-criterion \
    cargo-watch \
    cargo-expand

# Set up working directory in user's home (Cursor best practice)
USER ubuntu
WORKDIR /home/ubuntu

# Enable FUSE in container
RUN sudo modprobe fuse || true

# Default to bash for interactive development
CMD ["/bin/bash"]
