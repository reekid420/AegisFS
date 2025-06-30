// Build script for AegisFS - automatically detects OS and enables appropriate features

fn main() {
    // Automatically detect OS and enable appropriate filesystem support
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-cfg=windows_fs");
        // On Windows, we need WinFsp for filesystem operations
        println!("cargo:rustc-cfg=feature=\"winfsp\"");
        
        // Check if WinFsp is installed
        if std::env::var("WINFSP_INC").is_err() {
            println!("cargo:warning=WinFsp not found. Install WinFsp from https://winfsp.dev/ for filesystem mounting support");
        }
    } else if cfg!(any(target_os = "linux", target_os = "macos", target_os = "freebsd")) {
        println!("cargo:rustc-cfg=unix_fs");
        // On Unix systems, use FUSE
        println!("cargo:rustc-cfg=feature=\"fuse\"");
        
        // Check for FUSE availability
        if cfg!(target_os = "linux") {
            check_fuse_linux();
        } else if cfg!(target_os = "macos") {
            check_fuse_macos();
        }
    }
    
    // Always enable core features
    println!("cargo:rustc-cfg=feature=\"encryption\"");
    println!("cargo:rustc-cfg=feature=\"compression\"");
    
    // Print detected configuration
    println!("cargo:warning=AegisFS build configuration:");
    if cfg!(target_os = "windows") {
        println!("cargo:warning=  - Target OS: Windows");
        println!("cargo:warning=  - Filesystem: WinFsp");
    } else {
        println!("cargo:warning=  - Target OS: Unix/Linux");
        println!("cargo:warning=  - Filesystem: FUSE");
    }
}

fn check_fuse_linux() {
    // Check if FUSE development headers are available
    if std::process::Command::new("pkg-config")
        .args(&["--exists", "fuse3"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    {
        println!("cargo:warning=  - FUSE3 development headers: Found");
    } else if std::process::Command::new("pkg-config")
        .args(&["--exists", "fuse"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    {
        println!("cargo:warning=  - FUSE development headers: Found (legacy)");
    } else {
        println!("cargo:warning=  - FUSE development headers: Not found");
        println!("cargo:warning=    Install with: sudo apt-get install libfuse3-dev (Ubuntu/Debian)");
        println!("cargo:warning=    or: sudo yum install fuse3-devel (RHEL/Fedora)");
    }
}

fn check_fuse_macos() {
    // Check if macFUSE is available
    if std::path::Path::new("/usr/local/include/fuse").exists() 
        || std::path::Path::new("/opt/homebrew/include/fuse").exists() {
        println!("cargo:warning=  - macFUSE: Found");
    } else {
        println!("cargo:warning=  - macFUSE: Not found");
        println!("cargo:warning=    Install with: brew install macfuse");
    }
} 