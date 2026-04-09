# -----------------------------------------------------------------------------
# package-manager.sh
# Package installation utilities for build scripts.
# Focuses on Arch Linux with AUR support.
# -----------------------------------------------------------------------------

# --- Package manager --------------------------------------------------------

is_arch() {
    check_command pacman
}

# --- AUR helper -----------------------------------------------------------

has_aur_helper() {
    if check_command yay; then
        echo "yay"
    elif check_command paru; then
        echo "paru"
    else
        echo ""
    fi
}

# --- Installation ----------------------------------------------------------

install_pacman() {
    local package="$1"
    local name="${2:-$package}"
    
    if check_command "$package"; then
        return 1
    fi
    
    if ! is_arch; then
        return 2
    fi
    
    log_info "Installing $name..."
    if sudo pacman -S --noconfirm "$package" 2>/dev/null; then
        log_success "Installed $name"
        return 0
    else
        log_error "Failed to install $name"
        return 1
    fi
}

install_aur() {
    local package="$1"
    local name="${2:-$package}"
    local aur_helper
    aur_helper=$(has_aur_helper)
    
    if check_command "$package"; then
        return 1
    fi
    
    if [[ -z "$aur_helper" ]]; then
        log_error "No AUR helper (yay/paru)"
        return 2
    fi
    
    log_info "Installing $name from AUR..."
    $aur_helper -S --noconfirm "$package" 2>/dev/null
}

install_rust_target() {
    local target="$1"
    local name="$2"
    
    if check_rust_target_installed "$target"; then
        return 1
    fi
    
    log_info "Installing Rust target: $name..."
    if rustup target add "$target" 2>/dev/null; then
        log_success "Installed $target"
        return 0
    else
        log_error "Failed to install $target"
        return 1
    fi
}

install_cargo_tool() {
    local tool="$1"
    local name="${2:-$tool}"
    
    if check_command "$tool"; then
        return 1
    fi
    
    log_info "Installing cargo tool: $name..."
    if cargo install "$tool" 2>/dev/null; then
        log_success "Installed $name"
        return 0
    else
        log_error "Failed to install $name"
        return 1
    fi
}

install_from_url() {
    local url="$1"
    local extract_to="$2"
    local binary_name="$3"
    
    if check_command "$binary_name"; then
        return 1
    fi
    
    local temp_dir="/tmp/deltaspace-build"
    mkdir -p "$temp_dir"
    local archive="$temp_dir/$(basename "$url")"
    
    if ! download_file "$url" "$archive" "$binary_name"; then
        return 1
    fi
    
    mkdir -p "$extract_to"
    if ! extract_tarball "$archive" "$extract_to"; then
        log_error "Failed to extract $binary_name"
        rm -f "$archive"
        return 1
    fi
    
    rm -f "$archive"
    log_success "Installed $binary_name"
    return 0
}

# --- Verification ----------------------------------------------------------

verify_command() {
    local cmd="$1"
    local package="$2"
    
    if ! check_command "$cmd"; then
        log_error "$cmd not found"
        if [[ -n "$package" ]]; then
            echo "  Install with: sudo pacman -S $package"
        fi
        return 1
    fi
    return 0
}
