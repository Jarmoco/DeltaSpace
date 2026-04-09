/* -----------------------------------------------------------------------------
 * Filesystem Scanner
 *
 * Performs a depth-first traversal computing directory sizes from the
 * bottom up. Uses an iterative stack to avoid recursion limits on deep
 * filesystems. Two-pass approach: first collects all directories, then
 * computes sizes in reverse order (children before parents).
 * -------------------------------------------------------------------------- */

use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::Path,
};

fn normalize_path(path: &str) -> String {
    // Normalize paths to use forward slashes for consistent storage
    #[cfg(target_os = "windows")]
    return path.replace('\\', "/");
    #[cfg(not(target_os = "windows"))]
    return path.to_string();
}

pub fn scan(root: &str) -> HashMap<String, u64> {
    let mut sizes: HashMap<String, u64> = HashMap::new();
    let mut stack: Vec<String> = vec![normalize_path(root)];
    let mut order: Vec<String> = Vec::new();
    let mut counter: u64 = 0;

    /* Pass 1 - collect directories */
    while let Some(current_path) = stack.pop() {
        if crate::constants::is_excluded(&current_path) {
            continue;
        }
        order.push(current_path.clone());
        if let Ok(rd) = fs::read_dir(&current_path) {
            for entry in rd.flatten() {
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if !meta.file_type().is_symlink() && meta.is_dir() {
                    let p = normalize_path(&entry.path().to_string_lossy());
                    if !crate::constants::is_excluded(&p) {
                        stack.push(p);
                    }
                }
            }
        }
    }

    /* Pass 2 - compute sizes bottom-up */
    for current_path in order.iter().rev() {
        let mut total: u64 = 0;
        if let Ok(rd) = fs::read_dir(current_path) {
            for entry in rd.flatten() {
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if meta.file_type().is_symlink() {
                    continue;
                }
                if meta.is_dir() {
                    let p = normalize_path(&entry.path().to_string_lossy());
                    total += sizes.get(&p).copied().unwrap_or(0);
                } else {
                    total += meta.len();
                }
            }
        }
        sizes.insert(current_path.clone(), total);
        counter += 1;
        if counter % 200 == 0 {
            let base = Path::new(current_path)
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            print!("\r  {} dirs — {:<45}", counter, &base[..base.len().min(45)]);
            let _ = io::stdout().flush();
        }
    }

    sizes
}
