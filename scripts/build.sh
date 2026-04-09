# -----------------------------------------------------------------------------
# build.sh
# DeltaSpace build orchestrator.
# Modular build system supporting Linux, macOS (cross-compile), and Windows.
#
# Usage:
#   ./build.sh              Interactive mode
#   ./build.sh --target linux,macos,windows
#   ./build.sh --target all
#   ./build.sh --yes        Auto-yes to all prompts
#   ./build.sh --clean      Clean before building
# -----------------------------------------------------------------------------

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

source "$SCRIPT_DIR/lib/common.sh"
source "$SCRIPT_DIR/lib/package-manager.sh"

AUTO_YES=false
CLEAN_BUILD=false

TARGETS=()
BUILD_LINUX=true
BUILD_MACOS=true
BUILD_WINDOWS=true

# --- Help -----------------------------------------------------------------

show_help() {
    cat << 'EOF'
DeltaSpace Build Script
=======================

Usage:
  ./build.sh [options]

Options:
  -t, --target <platforms>   Build specific platforms (linux,macos,windows,all)
  -y, --yes                 Auto-answer yes to all prompts
  -c, --clean               Clean before building
  -h, --help                Show this help

Examples:
  ./build.sh                 Interactive mode (all platforms)
  ./build.sh -t linux        Build Linux only
  ./build.sh -t linux,macos  Build Linux and macOS
  ./build.sh -t all -y       Build all, auto-confirm

Note: Windows build on Windows just uses cargo build --release directly.
EOF
}

# --- Argument parsing -----------------------------------------------------

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -t|--target)
                IFS=',' read -ra TARGETS <<< "$2"
                shift 2
                ;;
            -y|--yes)
                AUTO_YES=true
                shift
                ;;
            -c|--clean)
                CLEAN_BUILD=true
                shift
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
    
    if [[ ${#TARGETS[@]} -gt 0 ]]; then
        BUILD_LINUX=false
        BUILD_MACOS=false
        BUILD_WINDOWS=false
        
        for target in "${TARGETS[@]}"; do
            case "$target" in
                linux) BUILD_LINUX=true ;;
                macos) BUILD_MACOS=true ;;
                windows) BUILD_WINDOWS=true ;;
                all)
                    BUILD_LINUX=true
                    BUILD_MACOS=true
                    BUILD_WINDOWS=true
                    ;;
                *)
                    log_error "Unknown target: $target"
                    exit 1
                    ;;
            esac
        done
    fi
}

# --- Platform selection ----------------------------------------------------

select_build_targets() {
    log_section "Build Targets"
    
    echo "Select platforms to build:"
    echo "  1) Linux"
    echo "  2) macOS"
    echo "  3) Windows"
    echo "  4) All platforms"
    echo ""
    echo "Or use command line: ./build.sh -t linux|macos|windows|all"
    
    local choice
    choice=$(ask_choice "Choose:" 1 2 3 4)
    
    BUILD_LINUX=false
    BUILD_MACOS=false
    BUILD_WINDOWS=false
    
    case "$choice" in
        0) BUILD_LINUX=true ;;
        1) BUILD_MACOS=true ;;
        2) BUILD_WINDOWS=true ;;
        3) BUILD_LINUX=true; BUILD_MACOS=true; BUILD_WINDOWS=true ;;
    esac
}

# --- Build summary ---------------------------------------------------------

show_build_summary() {
    log_section "Build Summary"
    
    echo "Version: $VERSION"
    echo ""
    echo "Build targets:"
    [[ "$BUILD_LINUX" == "true" ]]   && echo "  • Linux (x86_64)"
    [[ "$BUILD_MACOS" == "true" ]]   && echo "  • macOS (aarch64)"
    [[ "$BUILD_WINDOWS" == "true" ]] && echo "  • Windows (x86_64)"
    echo ""
    
    if [[ -d "$DIR_DIST" ]]; then
        echo "Output files:"
        ls -lh "$DIR_DIST/" 2>/dev/null | tail -n +2 | while read -r line; do
            echo "  $line"
        done
    else
        echo "No output files found"
    fi
}

# --- Pre-build checks -----------------------------------------------------

pre_build_checks() {
    log_section "Pre-build Checks"
    
    VERSION=$(get_version_from_cargo)
    if [[ -z "$VERSION" ]]; then
        log_error "Could not read version from Cargo.toml"
        exit 1
    fi
    log_info "Building version: $VERSION"
    
    if [[ "$CLEAN_BUILD" == "true" ]]; then
        log_info "Cleaning build directories..."
        cleanup_build_dirs
        log_success "Clean complete"
    fi
    
    log_success "Pre-build checks complete"
}

# --- Main -----------------------------------------------------------------

main() {
    parse_args "$@"
    
    setup_traps
    
    if [[ ${#TARGETS[@]} -eq 0 ]]; then
        select_build_targets
    fi
    
    if [[ "$BUILD_LINUX" != "true" ]] && \
       [[ "$BUILD_MACOS" != "true" ]] && \
       [[ "$BUILD_WINDOWS" != "true" ]]; then
        log_error "No build targets selected"
        exit 1
    fi
    
    pre_build_checks
    
    local build_failed=false
    local builds_succeeded=0
    local builds_attempted=0
    
    if [[ "$BUILD_LINUX" == "true" ]]; then
        builds_attempted=$((builds_attempted + 1))
        if "$SCRIPT_DIR/build-linux.sh"; then
            builds_succeeded=$((builds_succeeded + 1))
        else
            log_error "Linux build failed"
            build_failed=true
        fi
    fi
    
    if [[ "$BUILD_MACOS" == "true" ]]; then
        builds_attempted=$((builds_attempted + 1))
        if "$SCRIPT_DIR/build-macos.sh"; then
            builds_succeeded=$((builds_succeeded + 1))
        else
            log_error "macOS build failed"
            build_failed=true
        fi
    fi
    
    if [[ "$BUILD_WINDOWS" == "true" ]]; then
        builds_attempted=$((builds_attempted + 1))
        if "$SCRIPT_DIR/build-windows.sh"; then
            builds_succeeded=$((builds_succeeded + 1))
        else
            log_error "Windows build failed"
            build_failed=true
        fi
    fi
    
    show_build_summary
    
    if [[ "$build_failed" == "true" ]]; then
        echo ""
        log_error "Build completed with errors ($builds_succeeded/$builds_attempted successful)"
        exit 1
    fi
    
    echo ""
    log_success "All builds completed successfully ($builds_succeeded/$builds_attempted)"
    
    log_info "Done!"
    exit 0
}

main "$@"