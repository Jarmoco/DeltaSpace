
#!/bin/bash

# Ensure nfpm is installed
if ! command -v nfpm &> /dev/null; then
    echo "nfpm could not be found. Please install it first."
    exit 1
fi

# Ensure cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "cargo could not be found. Please install it first."
    exit 1
fi

# Create dist directory if it doesn't exist
mkdir -p ./dist

# Build the project
cargo build --release

# Create the packages
nfpm pkg --packager deb --target ./dist/
nfpm pkg --packager archlinux --target ./dist/
nfpm pkg --packager rpm --target ./dist/


