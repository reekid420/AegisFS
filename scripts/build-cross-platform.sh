#!/bin/bash

# Cross-platform build script for AegisFS
# Automatically detects OS and compiles with appropriate features

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Detect operating system
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    elif [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
        echo "windows"
    elif [[ "$OSTYPE" == "freebsd"* ]]; then
        echo "freebsd"
    else
        echo "unknown"
    fi
}

# Check if required tools are installed
check_dependencies() {
    print_status "Checking dependencies..."
    
    # Check for Rust
    if ! command -v cargo &> /dev/null; then
        print_error "Rust/Cargo is not installed. Please install from https://rustup.rs/"
        exit 1
    fi
    
    print_success "Rust/Cargo found: $(cargo --version)"
    
    # OS-specific dependency checks
    local os=$(detect_os)
    case $os in
        "linux")
            check_linux_deps
            ;;
        "macos")
            check_macos_deps
            ;;
        "windows")
            check_windows_deps
            ;;
        "freebsd")
            check_freebsd_deps
            ;;
        *)
            print_warning "Unknown OS detected. Proceeding with basic build..."
            ;;
    esac
}

check_linux_deps() {
    print_status "Checking Linux dependencies..."
    
    # Check for FUSE development headers
    if pkg-config --exists fuse3; then
        print_success "FUSE3 development headers found"
    elif pkg-config --exists fuse; then
        print_success "FUSE development headers found (legacy)"
    else
        print_warning "FUSE development headers not found"
        print_warning "Install with: sudo apt-get install libfuse3-dev (Ubuntu/Debian)"
        print_warning "           or: sudo yum install fuse3-devel (RHEL/Fedora)"
    fi
}

check_macos_deps() {
    print_status "Checking macOS dependencies..."
    
    # Check for macFUSE
    if [[ -d "/usr/local/include/fuse" ]] || [[ -d "/opt/homebrew/include/fuse" ]]; then
        print_success "macFUSE found"
    else
        print_warning "macFUSE not found"
        print_warning "Install with: brew install macfuse"
    fi
}

check_windows_deps() {
    print_status "Checking Windows dependencies..."
    
    # Check for WinFsp
    if [[ -n "$WINFSP_INC" ]]; then
        print_success "WinFsp found"
    else
        print_warning "WinFsp not found (environment variable WINFSP_INC not set)"
        print_warning "Install WinFsp from: https://winfsp.dev/"
    fi
}

check_freebsd_deps() {
    print_status "Checking FreeBSD dependencies..."
    
    # Check for FUSE
    if pkg info fusefs-libs >/dev/null 2>&1; then
        print_success "FUSE libraries found"
    else
        print_warning "FUSE libraries not found"
        print_warning "Install with: pkg install fusefs-libs"
    fi
}

# Build for the current platform
build_current_platform() {
    local os=$(detect_os)
    print_status "Detected OS: $os"
    
    # Build the core library first
    cd ../fs-core
    print_status "Building AegisFS core library..."
    cargo build --release --features "fuse,encryption,compression"
    cd ..
    
    # Build the unified CLI
    cd fs-app/cli
    print_status "Building AegisFS unified CLI..."
    
    case $os in
        "linux"|"freebsd")
            print_status "Building with FUSE support for Unix..."
            cargo build --release
            ;;
        "macos")
            print_status "Building with FUSE support for macOS..."
            cargo build --release
            ;;
        "windows")
            print_status "Building with WinFsp support for Windows..."
            # Note: WinFsp crate might not exist yet, so we build without filesystem mounting for now
            cargo build --release
            print_warning "Filesystem mounting not yet supported on Windows. File operations work normally."
            ;;
        *)
            print_status "Building with minimal features for unknown OS..."
            cargo build --release
            ;;
    esac
    
    cd ../..
}

# Cross-compile for specific targets
cross_compile() {
    local target=$1
    
    print_status "Cross-compiling for target: $target"
    
    # Install target if not already installed
    rustup target add $target
    
    # Build core library first
    cd fs-core
    print_status "Cross-compiling AegisFS core library for $target..."
    cargo build --release --target $target --features "fuse,encryption,compression"
    cd ..
    
    # Build unified CLI
    cd fs-app/cli
    print_status "Cross-compiling AegisFS CLI for $target..."
    
    case $target in
        *"windows"*)
            print_status "Cross-compiling for Windows..."
            cargo build --release --target $target
            ;;
        *"linux"*|*"unix"*)
            print_status "Cross-compiling for Linux/Unix..."
            cargo build --release --target $target
            ;;
        *"darwin"*)
            print_status "Cross-compiling for macOS..."
            cargo build --release --target $target
            ;;
        *)
            print_status "Cross-compiling with minimal features..."
            cargo build --release --target $target
            ;;
    esac
    
    cd ../..
}

# Show usage information
show_usage() {
    echo "AegisFS Cross-Platform Build Script"
    echo ""
    echo "Usage: $0 [COMMAND] [OPTIONS]"
    echo ""
    echo "Commands:"
    echo "  build                Build for current platform (default)"
    echo "  cross <target>       Cross-compile for specific target"
    echo "  test                 Run tests"
    echo "  clean                Clean build artifacts"
    echo "  deps                 Check dependencies only"
    echo ""
    echo "Common cross-compilation targets:"
    echo "  x86_64-pc-windows-msvc       Windows 64-bit"
    echo "  x86_64-unknown-linux-gnu     Linux 64-bit"
    echo "  x86_64-apple-darwin          macOS 64-bit"
    echo "  aarch64-apple-darwin         macOS ARM64"
    echo "  x86_64-unknown-freebsd       FreeBSD 64-bit"
    echo ""
    echo "Examples:"
    echo "  $0                                    # Build for current platform"
    echo "  $0 cross x86_64-pc-windows-msvc      # Cross-compile for Windows"
    echo "  $0 cross x86_64-unknown-linux-gnu    # Cross-compile for Linux"
    echo "  $0 test                               # Run tests"
}

# Main execution
main() {
    print_status "AegisFS Cross-Platform Build Script"
    print_status "===================================="
    
    case "${1:-build}" in
        "build")
            check_dependencies
            build_current_platform
            print_success "Build completed successfully!"
            print_status "AegisFS CLI binary is available in: fs-app/cli/target/release/aegisfs"
            print_status "Core library is available in: fs-core/target/release/"
            ;;
        "cross")
            if [[ -z "$2" ]]; then
                print_error "Target not specified for cross-compilation"
                show_usage
                exit 1
            fi
            check_dependencies
            cross_compile "$2"
            print_success "Cross-compilation completed successfully!"
            print_status "AegisFS CLI binary is available in: fs-app/cli/target/$2/release/aegisfs"
            print_status "Core library is available in: fs-core/target/$2/release/"
            ;;
        "test")
            check_dependencies
            cd fs-core
            print_status "Running core library tests..."
            cargo test --features "encryption,compression"
            cd ../fs-app/cli
            print_status "Running CLI tests..."
            cargo test
            cd ../..
            print_success "Tests completed successfully!"
            ;;
        "clean")
            cd fs-core
            print_status "Cleaning core library build artifacts..."
            cargo clean
            cd ../fs-app/cli
            print_status "Cleaning CLI build artifacts..."
            cargo clean
            cd ../..
            print_success "Clean completed!"
            ;;
        "deps")
            check_dependencies
            ;;
        "help"|"-h"|"--help")
            show_usage
            ;;
        *)
            print_error "Unknown command: $1"
            show_usage
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@" 