/* -----------------------------------------------------------------------------
 * Configuration Constants
 *
 * Centralizes all magic numbers and path configurations. This allows
 * the rest of the codebase to remain focused on logic rather than hardcoded
 * values scattered throughout.
 * -------------------------------------------------------------------------- */

pub const TARGET_DIR: &str = "/";
pub const THRESHOLD: u64 = 10 * 1024 * 1024;

pub fn get_output_dir() -> String {
    std::env::var("HOME")
        .map(|h| format!("{}/.local/share/deltaspace/snapshots", h))
        .unwrap_or_else(|_| "/var/log/deltaspace/snapshots".to_string())
}

pub const EXCLUDE_PREFIXES: &[&str] = &[
    "/proc",
    "/sys",
    "/dev",
    "/run",
    "/run/media",
    "/mnt",
    "/media",
];

#[allow(dead_code)]
pub fn is_excluded(path: &str) -> bool {
    for prefix in EXCLUDE_PREFIXES {
        if path == *prefix || path.starts_with(&format!("{}/", prefix)) {
            return true;
        }
    }
    false
}
