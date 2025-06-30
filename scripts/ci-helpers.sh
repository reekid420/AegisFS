#!/bin/bash
# CI/CD Helper Scripts for AegisFS
# This script provides utility functions for development and CI/CD operations

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running in CI environment
is_ci() {
    [[ "${CI:-false}" == "true" ]]
}

# Install system dependencies
install_system_deps() {
    log_info "Installing system dependencies..."
    
    if command -v apt-get >/dev/null 2>&1; then
        sudo apt-get update
        sudo apt-get install -y fuse3 libfuse3-dev pkg-config build-essential
        log_success "Installed dependencies via apt-get"
    elif command -v brew >/dev/null 2>&1; then
        brew install macfuse pkg-config
        log_success "Installed dependencies via brew"
    elif command -v yum >/dev/null 2>&1; then
        sudo yum install -y fuse3-devel pkgconfig gcc
        log_success "Installed dependencies via yum"
    else
        log_error "No supported package manager found"
        return 1
    fi
}

# Setup FUSE for testing
setup_fuse() {
    log_info "Setting up FUSE..."
    
    if is_ci; then
        sudo modprobe fuse || true
        sudo chmod 666 /dev/fuse || true
        sudo usermod -a -G fuse "$USER" || true
    else
        # For local development
        if ! grep -q fuse /proc/filesystems; then
            log_warning "FUSE not available in kernel, attempting to load module..."
            sudo modprobe fuse || {
                log_error "Failed to load FUSE module"
                return 1
            }
        fi
        
        if [[ ! -c /dev/fuse ]]; then
            log_error "/dev/fuse device not found"
            return 1
        fi
        
        if ! groups | grep -q fuse; then
            log_warning "User not in fuse group, you may need sudo for FUSE operations"
        fi
    fi
    
    log_success "FUSE setup complete"
}

# Run code formatting
format_code() {
    log_info "Formatting code..."
    cd "$(git rev-parse --show-toplevel)"
    
    cargo fmt --all
    log_success "Code formatting complete"
}

# Run linting
lint_code() {
    log_info "Running linting..."
    cd "$(git rev-parse --show-toplevel)"
    
    # Clippy with all features
    cargo clippy --all-targets --all-features -- -D warnings
    
    # Check documentation
    cargo doc --no-deps --all-features --document-private-items
    
    log_success "Linting complete"
}

# Run security audit
security_audit() {
    log_info "Running security audit..."
    cd "$(git rev-parse --show-toplevel)"
    
    # Install cargo-audit if not present
    if ! command -v cargo-audit >/dev/null 2>&1; then
        log_info "Installing cargo-audit..."
        cargo install cargo-audit
    fi
    
    # Install cargo-deny if not present
    if ! command -v cargo-deny >/dev/null 2>&1; then
        log_info "Installing cargo-deny..."
        cargo install cargo-deny
    fi
    
    cargo audit
    cargo deny check
    
    log_success "Security audit complete"
}

# Run tests with coverage
test_with_coverage() {
    log_info "Running tests with coverage..."
    cd "$(git rev-parse --show-toplevel)"
    
    # Install cargo-llvm-cov if not present
    if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
        log_info "Installing cargo-llvm-cov..."
        cargo install cargo-llvm-cov
    fi
    
    # Run unit tests with coverage
    cargo llvm-cov --all-features --workspace --lcov --output-path coverage.lcov
    
    log_success "Test coverage complete"
}

# Run integration tests
test_integration() {
    log_info "Running integration tests..."
    cd "$(git rev-parse --show-toplevel)"
    
    setup_fuse
    
    # Run integration tests with single thread to avoid FUSE conflicts
    cargo test --test persistence_test --test write_operations -- --test-threads=1
    
    log_success "Integration tests complete"
}

# Run benchmarks
run_benchmarks() {
    log_info "Running benchmarks..."
    cd "$(git rev-parse --show-toplevel)/fs-core"
    
    # Install criterion if not present
    if ! command -v cargo-criterion >/dev/null 2>&1; then
        log_info "Installing cargo-criterion..."
        cargo install cargo-criterion
    fi
    
    cargo criterion
    
    log_success "Benchmarks complete"
}

# Build release binaries
build_release() {
    local target="${1:-}"
    
    log_info "Building release binaries${target:+ for $target}..."
    cd "$(git rev-parse --show-toplevel)"
    
    if [[ -n "$target" ]]; then
        cargo build --release --target "$target" --all-features
    else
        cargo build --release --all-features
    fi
    
    log_success "Release build complete"
}

# Clean build artifacts
clean_build() {
    log_info "Cleaning build artifacts..."
    cd "$(git rev-parse --show-toplevel)"
    
    cargo clean
    
    # Clean Docker images and containers
    if command -v docker >/dev/null 2>&1; then
        docker system prune -f >/dev/null 2>&1 || true
    fi
    
    log_success "Build cleanup complete"
}

# Run full CI pipeline locally
run_full_ci() {
    log_info "Running full CI pipeline locally..."
    
    format_code
    lint_code
    security_audit
    test_with_coverage
    test_integration
    build_release
    
    log_success "Full CI pipeline complete!"
}

# Docker build helpers
docker_build() {
    local target="${1:-ci}"
    local tag="${2:-aegisfs:latest}"
    
    log_info "Building Docker image: $tag (target: $target)"
    cd "$(git rev-parse --show-toplevel)"
    
    docker build --target "$target" -t "$tag" .
    
    log_success "Docker build complete: $tag"
}

docker_test() {
    local image="${1:-aegisfs:latest}"
    
    log_info "Running tests in Docker container: $image"
    
    docker run --rm --privileged \
        -v /dev/fuse:/dev/fuse \
        "$image" cargo test --lib
    
    log_success "Docker tests complete"
}

# Help function
show_help() {
    cat << EOF
AegisFS CI/CD Helper Script

Usage: $0 <command> [options]

Commands:
    install-deps        Install system dependencies
    setup-fuse         Setup FUSE for testing
    format             Format code with rustfmt
    lint               Run clippy and doc checks
    audit              Run security audit
    test-coverage      Run tests with coverage
    test-integration   Run integration tests (requires FUSE)
    benchmarks         Run performance benchmarks
    build [target]     Build release binaries (optionally for specific target)
    clean              Clean build artifacts
    full-ci            Run complete CI pipeline locally
    docker-build [target] [tag]  Build Docker image
    docker-test [image]          Test in Docker container
    check-env          Check environment variables and CI/CD setup
    help               Show this help message

Examples:
    $0 full-ci                           # Run complete local CI
    $0 build x86_64-unknown-linux-musl   # Cross-compile for musl
    $0 docker-build dev aegisfs:dev      # Build development image
    $0 test-integration                  # Run integration tests

Environment:
    CI=true            Set to run in CI mode
    RUST_LOG=debug     Enable debug logging

EOF
}

# Main command dispatcher
main() {
    local command="${1:-help}"
    shift || true
    
    case "$command" in
        install-deps)
            install_system_deps
            ;;
        setup-fuse)
            setup_fuse
            ;;
        format)
            format_code
            ;;
        lint)
            lint_code
            ;;
        audit)
            security_audit
            ;;
        test-coverage)
            test_with_coverage
            ;;
        test-integration)
            test_integration
            ;;
        benchmarks)
            run_benchmarks
            ;;
        build)
            build_release "$@"
            ;;
        clean)
            clean_build
            ;;
        full-ci)
            run_full_ci
            ;;
        docker-build)
            docker_build "$@"
            ;;
        docker-test)
            docker_test "$@"
            ;;
        check-env)
            exec ./scripts/check-env.sh
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            log_error "Unknown command: $command"
            show_help
            exit 1
            ;;
    esac
}

# Only run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi 