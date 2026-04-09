# -----------------------------------------------------------------------------
# build-macos.sh
# macOS cross-compile script from Linux using cargo-zigbuild.
# Builds: macOS aarch64 (Apple Silicon) binary as tarball.
# -----------------------------------------------------------------------------

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"
source "$SCRIPT_DIR/lib/package-manager.sh"

if [[ -z "${VERSION:-}" ]]; then
    VERSION=$(get_version_from_cargo)
fi

# --- Dependency checks ----------------------------------------------------

check_dependencies() {
    log_section "macOS Build: Dependencies"
    
    if ! verify_command "cargo" "rust"; then
        log_error "Rust/Cargo is required"
        return 1
    fi
    
    if ! verify_command "zig" "zig"; then
        echo ""
        log_warn "zig not found"
        
        if is_arch; then
            if ! ask_yes_no "Install zig from AUR?"; then
                log_info "zig required for macOS cross-compile. Exiting."
                return 1
            fi
            
            if ! install_aur "zig" "zig"; then
                return 1
            fi
        else
            log_info "Trying direct download..."
            
            local extract_to="/opt"
            local binary_name="zig"
            
            if install_from_url "$URL_ZIG_X86_64" "$extract_to" "$binary_name"; then
                if [[ -L "/usr/local/bin/zig" ]]; then
                    sudo ln -sf "$extract_to/zig-linux-x86_64-$VERSION_ZIG/zig" /usr/local/bin/zig
                fi
                
                export PATH="$extract_to/zig-linux-x86_64-$VERSION_ZIG:$PATH"
            else
                log_error "Failed to install zig"
                return 1
            fi
        fi
    fi
    
    if ! verify_command "cargo-zigbuild" "cargo-zigbuild"; then
        echo ""
        log_warn "cargo-zigbuild not found"
        
        if ! ask_yes_no "Install cargo-zigbuild via cargo?"; then
            log_info "cargo-zigbuild required for macOS cross-compile. Exiting."
            return 1
        fi
        
        if ! install_cargo_tool "cargo-zigbuild" "cargo-zigbuild"; then
            return 1
        fi
    fi
    
    if ! check_rust_target_installed "aarch64-apple-darwin"; then
        echo ""
        log_warn "macOS aarch64 target not installed"
        
        if ! ask_yes_no "Install aarch64-apple-darwin target?"; then
            log_info "macOS target required for cross-compile. Exiting."
            return 1
        fi
        
        if ! install_rust_target "aarch64-apple-darwin" "aarch64-apple-darwin"; then
            return 1
        fi
    fi
    
    log_success "All dependencies satisfied"
    return 0
}

# --- Build steps ----------------------------------------------------------

build_macos_binary() {
    log_section "macOS Build: Compiling"
    
    log_info "Cross-compiling for macOS aarch64..."
    
    if ! cargo zigbuild --target aarch64-apple-darwin --release 2>&1; then
        log_error "cargo zigbuild failed"
        return 1
    fi
    
    BINARY="./target/aarch64-apple-darwin/release/deltaspace"
    if [[ ! -x "$BINARY" ]]; then
        log_error "Binary not found at $BINARY"
        return 1
    fi
    
    log_success "macOS binary compiled"
    return 0
}

package_macos_binary() {
    log_section "macOS Build: Packaging"
    
    ensure_dir "$DIR_DIST"
    
    log_info "Creating macOS tarball..."
    if [[ -n "${BINARY:-}" ]] && [[ -x "$BINARY" ]]; then
        tar -czf "./$DIR_DIST/deltaspace_${VERSION}_macos_aarch64.tar.gz" \
            -C "$(dirname "$BINARY")" deltaspace
        log_success "macOS tarball created"
    else
        log_error "Binary not found, cannot create tarball"
        return 1
    fi
    return 0
}

# --- Main -----------------------------------------------------------------

build_macos() {
    log_section "macOS Build"
    
    if ! check_dependencies; then
        return 1
    fi
    
    if ! build_macos_binary; then
        return 1
    fi
    
    if ! package_macos_binary; then
        return 1
    fi
    
    log_success "macOS build complete!"
    return 0
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    build_macos
fi
