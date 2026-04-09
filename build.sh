
#!/bin/bash

# -------------------------------------
# Check for package manager 
# -------------------------------------
# Check if pacman is available (for Arch Linux auto-install)
if command -v pacman &> /dev/null; then
    IS_ARCH=true
else
    IS_ARCH=false
fi

# -------------------------------------
# Setup tracking variables 
# -------------------------------------
PACKAGES_TO_CLEANUP=()

install_if_needed() {
    local cmd="$1"
    local package="$2"
    local name="$3"
    
    if command -v "$cmd" &> /dev/null; then
        return 1  # Already installed, don't track
    fi
    
    if [ "$IS_ARCH" = true ]; then
        echo "$name not found. Installing via pacman..."
        read -p "Install $name? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            sudo pacman -S --noconfirm "$package"
            PACKAGES_TO_CLEANUP+=("$package")
            echo "$name installed successfully."
            return 0
        else
            return 2  # User declined
        fi
    else
        return 2  # Not Arch, can't auto-install
    fi
}

# -------------------------------------
# Cleanup function
# -------------------------------------
cleanup_packages() {
    if [ ${#PACKAGES_TO_CLEANUP[@]} -eq 0 ]; then
        return
    fi
    
    echo ""
    read -p "Uninstall packages installed by this script? (y/n): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Uninstalling packages..."
        for pkg in "${PACKAGES_TO_CLEANUP[@]}"; do
            sudo pacman -R --noconfirm "$pkg" 2>/dev/null || true
        done
        echo "Cleanup complete."
    else
        echo "Packages left installed. Remove manually with:"
        echo "  sudo pacman -R ${PACKAGES_TO_CLEANUP[*]}"
    fi
}

# -------------------------------------
# Main build flow
# -------------------------------------

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
    if [ "$IS_ARCH" = true ]; then
        echo "nfpm could not be found."
        read -p "Install nfpm? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            # Try to install nfpm from AUR (may need makepkg)
            if command -v yay &> /dev/null; then
                yay -S --noconfirm nfpm
                PACKAGES_TO_CLEANUP+=("nfpm")
            elif command -v makepkg &> /dev/null; then
                cd /tmp
                git clone https://aur.archlinux.org/nfpm.git
                cd nfpm
                makepkg -si --noconfirm
                PACKAGES_TO_CLEANUP+=("nfpm")
                cd -
            else
                echo "Neither yay nor makepkg found. Cannot build nfpm from AUR."
                exit 1
            fi
        else
            echo "nfpm is required for Linux build. Exiting."
            exit 1
        fi
    else
        echo "nfpm could not be found. Please install it first."
        exit 1
    fi
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
    echo "zig could not be found."
    if [ "$IS_ARCH" = true ]; then
        read -p "Install zig? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            # Check for yay/makepkg or use direct download
            if command -v yay &> /dev/null; then
                yay -S --noconfirm zig
            elif command -v makepkg &> /dev/null; then
                cd /tmp
                git clone https://aur.archlinux.org/zig.git
                cd zig
                makepkg -si --noconfirm
                cd -
            else
                # Try direct download
                curl -L https://ziglang.org/download/0.14.0/zig-linux-x86_64.tar.xz -o /tmp/zig.tar.xz
                sudo tar -xf /tmp/zig.tar.xz -C /opt
                sudo ln -sf /opt/zig-linux-x86_64-0.14.0/zig /usr/local/bin/zig
            fi
            PACKAGES_TO_CLEANUP+=("zig")
        else
            echo "zig is required for macOS cross-compilation. Exiting."
            exit 1
        fi
    else
        echo "zig could not be found. Please install it first."
        exit 1
    fi
fi

# Ensure cargo-zigbuild is installed
if ! command -v cargo-zigbuild &> /dev/null; then
    echo "cargo-zigbuild could not be found."
    if [ "$IS_ARCH" = true ]; then
        read -p "Install cargo-zigbuild? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            cargo install cargo-zigbuild
            # Note: cargo install doesn't add to pacman, so we track differently
            # Just note it for user
            echo "cargo-zigbuild installed (via cargo install)."
        else
            echo "cargo-zigbuild is required for macOS cross-compilation. Exiting."
            exit 1
        fi
    else
        echo "cargo-zigbuild could not be found. Please install it first."
        exit 1
    fi
fi

# Ensure rustup target add aarch64-apple-darwin was run
if ! rustup target list | grep "aarch64-apple-darwin (installed)" > /dev/null; then
    echo "aarch64-apple-darwin target not found."
    if [ "$IS_ARCH" = true ]; then
        read -p "Install aarch64-apple-darwin target? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            rustup target add aarch64-apple-darwin
            # Track for potential cleanup (though rustup targets are small)
            echo "aarch64-apple-darwin target installed."
        else
            echo "aarch64-apple-darwin target is required for macOS cross-compilation. Exiting."
            exit 1
        fi
    else
        echo "Please run 'rustup target add aarch64-apple-darwin' first."
        exit 1
    fi
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
    echo "x86_64-pc-windows-gnu target not found."
    if [ "$IS_ARCH" = true ]; then
        read -p "Install x86_64-pc-windows-gnu target? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            rustup target add x86_64-pc-windows-gnu
        else
            echo "x86_64-pc-windows-gnu target is required for Windows build. Exiting."
            exit 1
        fi
    else
        echo "Please run 'rustup target add x86_64-pc-windows-gnu' first."
        exit 1
    fi
fi

# Check for mingw-w64 (required for Windows GNU target)
if ! command -v x86_64-w64-mingw32-gcc &> /dev/null; then
    echo "x86_64-w64-mingw32-gcc could not be found."
    
    # Detect OS and offer to install mingw-w64
    if [ "$IS_ARCH" = true ]; then
        echo "Detected Arch Linux. Installing mingw-w64..."
        read -p "Install mingw-w64? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            sudo pacman -S --noconfirm mingw-w64-binutils mingw-w64-gcc mingw-w64-headers mingw-w64-winpthreads
            MINGW_INSTALLED_BY_SCRIPT=true
            PACKAGES_TO_CLEANUP+=("mingw-w64-binutils" "mingw-w64-gcc" "mingw-w64-headers" "mingw-w64-winpthreads")
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

# -------------------------------------
# Build complete
# -------------------------------------

echo ""
echo "Build complete! Packages available in ./dist/"
echo "Files created:"
ls -lh ./dist/

# Ask to cleanup packages if any were installed
if [ ${#PACKAGES_TO_CLEANUP[@]} -gt 0 ]; then
    cleanup_packages
fi
