
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