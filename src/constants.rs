/* -----------------------------------------------------------------------------
 * Configuration Constants
 *
 * Centralizes all magic numbers and path configurations. This allows
 * the rest of the codebase to remain focused on logic rather than hardcoded
 * values scattered throughout.
 * -------------------------------------------------------------------------- */

pub const THRESHOLD: u64 = 10 * 1024 * 1024;

/* --- Platform-specific paths ---------------------------------------------- */

#[cfg(not(target_os = "windows"))]
pub const TARGET_DIR: &str = "/";

#[cfg(target_os = "windows")]
pub const TARGET_DIR: &str = "C:\\";

#[cfg(not(target_os = "windows"))]
pub fn get_output_dir() -> String {
    std::env::var("HOME")
        .map(|h| format!("{}/.local/share/deltaspace/snapshots", h))
        .unwrap_or_else(|_| "/var/log/deltaspace/snapshots".to_string())
}

#[cfg(target_os = "windows")]
pub fn get_output_dir() -> String {
    std::env::var("LOCALAPPDATA")
        .map(|h| format!("{}\\deltaspace\\snapshots", h))
        .unwrap_or_else(|_| "C:\\ProgramData\\deltaspace\\snapshots".to_string())
}

/* --- Platform-specific exclusions ----------------------------------------- */

#[cfg(not(target_os = "windows"))]
pub const EXCLUDE_PREFIXES: &[&str] = &[
    "/proc",
    "/sys",
    "/dev",
    "/run",
    "/run/media",
    "/mnt",
    "/media",
];

#[cfg(target_os = "windows")]
pub const EXCLUDE_PREFIXES: &[&str] = &[
    "C:\\Windows",
    "C:\\Program Files",
    "C:\\Program Files (x86)",
    "C:\\$Recycle.Bin",
    "C:\\System Volume Information",
];

/* --- Exclusion checking --------------------------------------------------- */

#[allow(dead_code)]
pub fn is_excluded(path: &str) -> bool {
    #[cfg(not(target_os = "windows"))]
    let separator = "/";
    #[cfg(target_os = "windows")]
    let separator = "\\";

    for prefix in EXCLUDE_PREFIXES {
        if path == *prefix || path.starts_with(&format!("{}{}", prefix, separator)) {
            return true;
        }
    }
    false
}
