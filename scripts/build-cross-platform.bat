@echo off
setlocal enabledelayedexpansion

REM Cross-platform build script for AegisFS (Windows version)
REM Automatically detects OS and compiles with appropriate features

echo [INFO] AegisFS Cross-Platform Build Script
echo [INFO] ====================================

REM Set default command
set COMMAND=%1
if "%COMMAND%"=="" set COMMAND=build

REM Main execution
if "%COMMAND%"=="build" goto :build
if "%COMMAND%"=="cross" goto :cross
if "%COMMAND%"=="test" goto :test
if "%COMMAND%"=="clean" goto :clean
if "%COMMAND%"=="deps" goto :deps
if "%COMMAND%"=="help" goto :usage
if "%COMMAND%"=="-h" goto :usage
if "%COMMAND%"=="--help" goto :usage

echo [ERROR] Unknown command: %COMMAND%
goto :usage

:build
echo [INFO] Building for Windows platform...
call :check_dependencies
if errorlevel 1 exit /b 1

REM Build core library first
cd ../fs-core
echo [INFO] Building AegisFS core library...
cargo build --release --features "encryption,compression"
if errorlevel 1 (
    echo [ERROR] Core library build failed
    cd ..
    exit /b 1
)
cd ..

REM Build unified CLI
cd fs-app\cli
echo [INFO] Building AegisFS unified CLI...
echo [WARNING] Filesystem mounting not yet fully supported on Windows
cargo build --release
if errorlevel 1 (
    echo [ERROR] CLI build failed
    cd ..\..
    exit /b 1
)
cd ..\..

echo [SUCCESS] Build completed successfully!
echo [INFO] AegisFS CLI binary is available in: fs-app\cli\target\release\aegisfs.exe
echo [INFO] Core library is available in: fs-core\target\release\
goto :end

:cross
if "%2"=="" (
    echo [ERROR] Target not specified for cross-compilation
    goto :usage
)
set TARGET=%2
echo [INFO] Cross-compiling for target: %TARGET%

call :check_dependencies
if errorlevel 1 exit /b 1

echo [INFO] Installing target if not already installed...
rustup target add %TARGET%
if errorlevel 1 (
    echo [ERROR] Failed to add target %TARGET%
    exit /b 1
)

REM Build core library first
cd fs-core
echo [INFO] Cross-compiling AegisFS core library for %TARGET%...

if "%TARGET:windows=%" neq "%TARGET%" (
    echo [INFO] Cross-compiling for Windows...
    cargo build --release --target %TARGET% --features "encryption,compression"
) else if "%TARGET:linux=%" neq "%TARGET%" (
    echo [INFO] Cross-compiling for Linux...
    cargo build --release --target %TARGET% --features "fuse,encryption,compression"
) else if "%TARGET:darwin=%" neq "%TARGET%" (
    echo [INFO] Cross-compiling for macOS...
    cargo build --release --target %TARGET% --features "fuse,encryption,compression"
) else (
    echo [INFO] Cross-compiling with minimal features...
    cargo build --release --target %TARGET% --features "encryption,compression"
)

if errorlevel 1 (
    echo [ERROR] Core library cross-compilation failed
    cd ..
    exit /b 1
)
cd ..

REM Build unified CLI
cd fs-app\cli
echo [INFO] Cross-compiling AegisFS CLI for %TARGET%...
cargo build --release --target %TARGET%
if errorlevel 1 (
    echo [ERROR] CLI cross-compilation failed
    cd ..\..
    exit /b 1
)
cd ..\..

echo [SUCCESS] Cross-compilation completed successfully!
echo [INFO] AegisFS CLI binary is available in: fs-app\cli\target\%TARGET%\release\aegisfs.exe
echo [INFO] Core library is available in: fs-core\target\%TARGET%\release\
goto :end

:test
echo [INFO] Running tests...
call :check_dependencies
if errorlevel 1 exit /b 1

REM Run core library tests
cd fs-core
echo [INFO] Running core library tests...
cargo test --features "encryption,compression"
if errorlevel 1 (
    echo [ERROR] Core library tests failed
    cd ..
    exit /b 1
)
cd ..

REM Run CLI tests
cd fs-app\cli
echo [INFO] Running CLI tests...
cargo test
if errorlevel 1 (
    echo [ERROR] CLI tests failed
    cd ..\..
    exit /b 1
)
cd ..\..

echo [SUCCESS] Tests completed successfully!
goto :end

:clean
echo [INFO] Cleaning build artifacts...

REM Clean core library
cd fs-core
echo [INFO] Cleaning core library build artifacts...
cargo clean
cd ..

REM Clean CLI
cd fs-app\cli
echo [INFO] Cleaning CLI build artifacts...
cargo clean
cd ..\..

echo [SUCCESS] Clean completed!
goto :end

:deps
call :check_dependencies
goto :end

:check_dependencies
echo [INFO] Checking dependencies...

REM Check for Rust/Cargo
where cargo >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Rust/Cargo is not installed. Please install from https://rustup.rs/
    exit /b 1
)

for /f "delims=" %%i in ('cargo --version') do set CARGO_VERSION=%%i
echo [SUCCESS] Rust/Cargo found: !CARGO_VERSION!

REM Check for WinFsp
if defined WINFSP_INC (
    echo [SUCCESS] WinFsp found
) else (
    echo [WARNING] WinFsp not found (environment variable WINFSP_INC not set)
    echo [WARNING] Install WinFsp from: https://winfsp.dev/
    echo [WARNING] Note: WinFsp is only needed for filesystem mounting functionality
)

REM Check if Visual Studio Build Tools are available
where cl >nul 2>&1
if errorlevel 1 (
    echo [WARNING] Visual Studio Build Tools not found in PATH
    echo [WARNING] Some features may require MSVC compiler
    echo [WARNING] Install from: https://visualstudio.microsoft.com/visual-cpp-build-tools/
) else (
    echo [SUCCESS] Visual Studio Build Tools found
)

exit /b 0

:usage
echo AegisFS Cross-Platform Build Script (Windows)
echo.
echo Usage: %0 [COMMAND] [OPTIONS]
echo.
echo Commands:
echo   build                Build for current platform (default)
echo   cross ^<target^>       Cross-compile for specific target
echo   test                 Run tests
echo   clean                Clean build artifacts
echo   deps                 Check dependencies only
echo.
echo Common cross-compilation targets:
echo   x86_64-pc-windows-msvc       Windows 64-bit
echo   x86_64-unknown-linux-gnu     Linux 64-bit
echo   x86_64-apple-darwin          macOS 64-bit
echo   aarch64-apple-darwin         macOS ARM64
echo   x86_64-unknown-freebsd       FreeBSD 64-bit
echo.
echo Examples:
echo   %0                                    # Build for current platform
echo   %0 cross x86_64-pc-windows-msvc      # Cross-compile for Windows
echo   %0 cross x86_64-unknown-linux-gnu    # Cross-compile for Linux
echo   %0 test                               # Run tests
goto :end

:end 
:end 