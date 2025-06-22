# Base image with Rust and common build tools
FROM rust:latest

# Install system dependencies
RUN apt-get update && apt-get install -y \
    clang \
    cmake \
    fuse \
    libfuse-dev \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust components
RUN rustup component add rustfmt clippy

# Create non-root user
RUN useradd -ms /bin/bash developer
USER developer
WORKDIR /home/developer/workspace

# Pre-build cargo registry for faster builds
RUN cargo init --bin dummy && \
    cd dummy && \
    echo 'fuser = "0.12"' >> Cargo.toml && \
    cargo build --release && \
    cd .. && \
    rm -rf dummy

# Default command
CMD ["/bin/bash"]
