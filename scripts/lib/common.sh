# -----------------------------------------------------------------------------
# common.sh
# Shared utilities for the build scripts.
# Provides: constants, logging, download utilities, error handling.
# -----------------------------------------------------------------------------

# --- Configuration constants ----------------------------------------------

readonly VERSION_ZIG="0.14.0"
readonly VERSION_NFPM="2.34.2"

readonly URL_ZIG_X86_64="https://ziglang.org/download/${VERSION_ZIG}/zig-linux-x86_64-${VERSION_ZIG}.tar.xz"
readonly URL_ZIG_AARCH64="https://ziglang.org/download/${VERSION_ZIG}/zig-linux-aarch64-${VERSION_ZIG}.tar.xz"

readonly DIR_DIST="./dist"
readonly DIR_TARGET="./target"

# --- Colors ----------------------------------------------------------------

readonly COLOR_RESET="\033[0m"
readonly COLOR_RED="\033[0;31m"
readonly COLOR_GREEN="\033[0;32m"
readonly COLOR_YELLOW="\033[0;33m"
readonly COLOR_BLUE="\033[0;34m"
readonly COLOR_BOLD="\033[1m"

# --- Logging ---------------------------------------------------------------

log_info() {
    echo -e "${COLOR_BLUE}[INFO]${COLOR_RESET} $*"
}

log_success() {
    echo -e "${COLOR_GREEN}[OK]${COLOR_RESET} $*"
}

log_warn() {
    echo -e "${COLOR_YELLOW}[WARN]${COLOR_RESET} $*" >&2
}

log_error() {
    echo -e "${COLOR_RED}[ERROR]${COLOR_RESET} $*" >&2
}

log_section() {
    echo ""
    echo -e "${COLOR_BOLD}━━━ $* ━━━${COLOR_RESET}"
}

# --- User interaction ------------------------------------------------------

ask_yes_no() {
    local prompt="$1"
    local default="${2:-n}"
    
    if [[ "$AUTO_YES" == "true" ]]; then
        return 0
    fi
    
    local choices="[y/N]"
    if [[ "$default" == "y" ]]; then
        choices="[Y/n]"
    fi
    
    while true; do
        read -p "$prompt $choices: " -n 1 -r reply
        echo
        
        if [[ -z "$reply" ]]; then
            reply="$default"
        fi
        
        if [[ "$reply" =~ ^[Yy]$ ]]; then
            return 0
        elif [[ "$reply" =~ ^[Nn]$ ]]; then
            return 1
        fi
        
        echo "Please answer y or n"
    done
}

ask_choice() {
    local prompt="$1"
    shift
    local options=("$@")
    local num=${#options[@]}
    
    if [[ "$AUTO_YES" == "true" ]]; then
        echo "0"
        return
    fi
    
    echo -n "$prompt " >&2
    local line
    while IFS= read -r line || [[ -n "$line" ]]; do
        if [[ "$line" =~ ^[0-9]+$ ]] && [[ "$line" -ge 1 ]] && [[ "$line" -le "$num" ]]; then
            echo $((line-1))
            return 0
        fi
        echo "Invalid choice. Please enter 1-$num" >&2
        echo -n "$prompt " >&2
    done
    return 1
}

# --- Download utilities ----------------------------------------------------

download_file() {
    local url="$1"
    local dest="$2"
    local name="$3"
    
    log_info "Downloading $name..."
    
    if command -v curl &>/dev/null; then
        curl -fSL --connect-timeout 30 --retry 3 --retry-delay 2 \
            -o "$dest" "$url" 2>/dev/null
    elif command -v wget &>/dev/null; then
        wget -q --connect-timeout=30 --tries=3 --wait=2 -O "$dest" "$url" 2>/dev/null
    else
        log_error "Neither curl nor wget available"
        return 1
    fi
    
    if [[ $? -eq 0 ]] && [[ -s "$dest" ]]; then
        log_success "Downloaded $name"
        return 0
    else
        log_error "Failed to download $name"
        rm -f "$dest"
        return 1
    fi
}

extract_tarball() {
    local archive="$1"
    local dest="$2"
    
    if [[ "$archive" == *.tar.xz ]]; then
        tar -xJf "$archive" -C "$dest"
    elif [[ "$archive" == *.tar.gz ]] || [[ "$archive" == *.tgz ]]; then
        tar -xzf "$archive" -C "$dest"
    elif [[ "$archive" == *.tar ]]; then
        tar -xf "$archive" -C "$dest"
    else
        log_error "Unknown archive format: $archive"
        return 1
    fi
}

# --- File utilities --------------------------------------------------------

ensure_dir() {
    local dir="$1"
    if [[ ! -d "$dir" ]]; then
        mkdir -p "$dir"
        log_info "Created directory: $dir"
    fi
}

clean_dir() {
    local dir="$1"
    if [[ -d "$dir" ]]; then
        log_info "Cleaning $dir"
        rm -rf "$dir"
    fi
}

cleanup_build_dirs() {
    log_info "Cleaning build directories..."
    clean_dir "$DIR_DIST"
    clean_dir "$DIR_TARGET"
}

# --- Version utilities -----------------------------------------------------

get_version_from_cargo() {
    local toml="Cargo.toml"
    if [[ ! -f "$toml" ]]; then
        log_error "Cargo.toml not found"
        return 1
    fi
    
    grep -m1 '^version' "$toml" | sed 's/version = "\(.*\)"/\1/'
}

# --- Check utilities -------------------------------------------------------

check_command() {
    local cmd="$1"
    command -v "$cmd" &>/dev/null
}

check_file() {
    local file="$1"
    [[ -f "$file" ]]
}

check_rust_target_installed() {
    local target="$1"
    rustup target list 2>/dev/null | grep -q "$target (installed)"
}

# --- Error handling --------------------------------------------------------

handle_error() {
    local line=$1
    local exit_code=$?
    
    echo ""
    log_error "Build failed at line $line (exit code: $exit_code)"
    echo ""
    echo "Partial build artifacts may remain in ./dist/"
    echo "You can run 'rm -rf ./dist ./target' to clean up manually."
    exit "$exit_code"
}

setup_traps() {
    trap 'handle_error $LINENO' ERR
    trap 'log_info "Interrupted"; exit 130' INT
    trap 'log_info "Terminated"; exit 143' TERM
}
