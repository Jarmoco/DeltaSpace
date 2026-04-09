
#!/bin/bash

# Prompt user for version
read -p "Enter version: " VERSION

# Clean previous builds
rm -rf ./dist
rm -rf ./target

# -------------------------------------
# Linux build 
# -------------------------------------

# Ensure cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "cargo could not be found. Please install it first."
    exit 1
fi

# Ensure nfpm is installed
if ! command -v nfpm &> /dev/null; then
    echo "nfpm could not be found. Please install it first."
    exit 1
fi

# Create dist directory if it doesn't exist
mkdir -p ./dist

# Build the project for linux
cargo build --release

# Create the packages
nfpm pkg --packager deb --target ./dist/
nfpm pkg --packager archlinux --target ./dist/
nfpm pkg --packager rpm --target ./dist/

# Package a generic linux binary
tar -czf ./dist/deltaspace_${VERSION}_linux_x86_64.tar.gz ./target/release/deltaspace

# -------------------------------------
# MacOS build 
# -------------------------------------

# Ensure zig is installed 
if ! command -v zig &> /dev/null; then
  echo "zig could not be found. Please install it first."
  exit 1
fi 

# Ensure cargo-zigbuild is installed
if ! command -v cargo-zigbuild &> /dev/null; then
  echo "cargo-zigbuild could not be found. Please install it first."
  exit 1
fi 

# Ensure rustup target add aarch64-apple-darwin was run
if ! rustup target list | grep "aarch64-apple-darwin (installed)" > /dev/null; then
  echo "aarch64-apple-darwin target not found. Please run 'rustup target add aarch64-apple-darwin' first."
  exit 1
fi 

cargo zigbuild --target aarch64-apple-darwin --release

# Package the built binary (target/aarch64-apple-darwin/release/deltaspace) into tar.gz
tar -czf ./dist/deltaspace_${VERSION}_macos_aarch64.tar.gz ./target/aarch64-apple-darwin/release/deltaspace

# -------------------------------------
# Windows build 
# -------------------------------------
MINGW_INSTALLED_BY_SCRIPT=false

# Ensure rustup target for Windows GNU is installed
if ! rustup target list | grep "x86_64-pc-windows-gnu (installed)" > /dev/null; then
    echo "x86_64-pc-windows-gnu target not found. Installing..."
    rustup target add x86_64-pc-windows-gnu
fi

# Check for mingw-w64 (required for Windows GNU target)
if ! command -v x86_64-w64-mingw32-gcc &> /dev/null; then
    echo "x86_64-w64-mingw32-gcc could not be found."
    
    # Detect OS and offer to install mingw-w64
    if [ -f /etc/arch-release ]; then
        echo "Detected Arch Linux. Installing mingw-w64..."
        read -p "Install mingw-w64? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            sudo pacman -S --noconfirm mingw-w64-binutils mingw-w64-gcc mingw-w64-headers mingw-w64-winpthreads
            MINGW_INSTALLED_BY_SCRIPT=true
            echo "mingw-w64 installed successfully."
        else
            echo "mingw-w64 is required for Windows build. Exiting."
            exit 1
        fi
    else
        echo "Please install mingw-w64 manually:"
        echo "  Arch Linux: sudo pacman -S mingw-w64-binutils mingw-w64-gcc mingw-w64-headers mingw-w64-winpthreads"
        echo "  Debian/Ubuntu: sudo apt-get install mingw-w64"
        echo "  macOS: brew install mingw-w64"
        exit 1
    fi
fi

# Build for Windows (x86_64 GNU)
echo "Building for Windows x86_64..."
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    cargo build --target x86_64-pc-windows-gnu --release

# Package the Windows binary
if [ -f "./target/x86_64-pc-windows-gnu/release/deltaspace.exe" ]; then
    cp "./target/x86_64-pc-windows-gnu/release/deltaspace.exe" "./dist/deltaspace_${VERSION}_windows_x86_64.exe"
    echo "Windows build complete: deltaspace_${VERSION}_windows_x86_64.exe"
else
    echo "Warning: Windows binary not found at expected location"
fi

echo ""
echo "Build complete! Packages available in ./dist/"
echo "Files created:"
ls -lh ./dist/

# Ask to uninstall mingw-w64 if installed by this script
if [ "$MINGW_INSTALLED_BY_SCRIPT" = true ]; then
    echo ""
    read -p "Uninstall mingw-w64 (installed by this script)? (y/n): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Uninstalling mingw-w64..."
        sudo pacman -R --noconfirm mingw-w64-binutils mingw-w64-gcc mingw-w64-headers mingw-w64-winpthreads 2>/dev/null || true
        echo "mingw-w64 uninstalled."
    else
        echo "mingw-w64 left installed. You can remove it manually with:"
        echo "  sudo pacman -R mingw-w64-binutils mingw-w64-gcc mingw-w64-headers mingw-w64-winpthreads"
    fi
fi
