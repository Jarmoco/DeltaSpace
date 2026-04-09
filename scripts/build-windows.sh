# -----------------------------------------------------------------------------
# build-windows.sh
# Windows cross-compile script from Linux using mingw-w64.
# Builds: Windows x86_64 .exe binary.
# Note: On Windows, just run 'cargo build --release' instead.
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
    log_section "Windows Build: Dependencies"
    
    if ! verify_command "cargo" "rust"; then
        log_error "Rust/Cargo is required"
        return 1
    fi
    
    if ! check_rust_target_installed "x86_64-pc-windows-gnu"; then
        echo ""
        log_warn "Windows GNU target not installed"
        
        if ! ask_yes_no "Install x86_64-pc-windows-gnu target?"; then
            log_info "Windows target required for cross-compile. Exiting."
            return 1
        fi
        
        if ! install_rust_target "x86_64-pc-windows-gnu" "x86_64-pc-windows-gnu"; then
            return 1
        fi
    fi
    
    if ! verify_command "x86_64-w64-mingw32-gcc" "mingw-w64"; then
        echo ""
        log_warn "mingw-w64 not found"
        
        if is_arch; then
            if ! ask_yes_no "Install mingw-w64?"; then
                log_info "mingw-w64 required for Windows cross-compile. Exiting."
                return 1
            fi
            
            log_info "Installing mingw-w64 packages..."
            local packages=(
                "mingw-w64-binutils"
                "mingw-w64-gcc"
                "mingw-w64-headers"
                "mingw-w64-winpthreads"
            )
            
            if sudo pacman -S --noconfirm "${packages[@]}" 2>/dev/null; then
                log_success "mingw-w64 installed"
            else
                log_error "Failed to install mingw-w64"
                return 1
            fi
        else
            log_error "mingw-w64 not available on this system"
            return 1
        fi
    fi
    
    log_success "All dependencies satisfied"
    return 0
}

# --- Build steps ----------------------------------------------------------

build_windows_binary() {
    log_section "Windows Build: Compiling"
    
    log_info "Cross-compiling for Windows x86_64..."
    
    if ! CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
         cargo build --target x86_64-pc-windows-gnu --release 2>&1; then
        log_error "Windows cross-compile failed"
        return 1
    fi
    
    BINARY="./target/x86_64-pc-windows-gnu/release/deltaspace.exe"
    if [[ ! -f "$BINARY" ]]; then
        log_error "Binary not found at $BINARY"
        return 1
    fi
    
    log_success "Windows binary compiled"
    return 0
}

package_windows_binary() {
    log_section "Windows Build: Packaging"
    
    ensure_dir "$DIR_DIST"
    
    local dest="$DIR_DIST/deltaspace_${VERSION}_windows_x86_64.exe"
    
    log_info "Copying Windows binary..."
    if [[ -n "${BINARY:-}" ]] && [[ -f "$BINARY" ]]; then
        cp "$BINARY" "$dest"
        log_success "Windows .exe created: $dest"
    else
        log_error "Binary not found, cannot create package"
        return 1
    fi
    return 0
}

# --- Main -----------------------------------------------------------------

build_windows() {
    log_section "Windows Build"
    
    if ! check_dependencies; then
        return 1
    fi
    
    if ! build_windows_binary; then
        return 1
    fi
    
    if ! package_windows_binary; then
        return 1
    fi
    
    log_success "Windows build complete!"
    return 0
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    build_windows
fi
