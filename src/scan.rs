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
    os::unix::fs::MetadataExt,
    path::Path,
};

pub fn scan(root: &str) -> HashMap<String, u64> {
    let mut sizes: HashMap<String, u64> = HashMap::new();
    let mut stack: Vec<String> = vec![root.to_string()];
    let mut order: Vec<String> = Vec::new();
    let mut counter: u64 = 0;

    /* Pass 1 - collect directories */
    while let Some(cur) = stack.pop() {
        if crate::constants::is_excluded(&cur) {
            continue;
        }
        order.push(cur.clone());
        if let Ok(rd) = fs::read_dir(&cur) {
            for entry in rd.flatten() {
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if !meta.file_type().is_symlink() && meta.is_dir() {
                    let p = entry.path().to_string_lossy().into_owned();
                    if !crate::constants::is_excluded(&p) {
                        stack.push(p);
                    }
                }
            }
        }
    }

    /* Pass 2 - compute sizes bottom-up */
    for cur in order.iter().rev() {
        let mut total: u64 = 0;
        if let Ok(rd) = fs::read_dir(cur) {
            for entry in rd.flatten() {
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if meta.file_type().is_symlink() {
                    continue;
                }
                if meta.is_dir() {
                    let p = entry.path().to_string_lossy().into_owned();
                    total += sizes.get(&p).copied().unwrap_or(0);
                } else {
                    total += meta.size();
                }
            }
        }
        sizes.insert(cur.clone(), total);
        counter += 1;
        if counter % 200 == 0 {
            let base = Path::new(cur)
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            print!("\r  {} dirs — {:<45}", counter, &base[..base.len().min(45)]);
            let _ = io::stdout().flush();
        }
    }

    sizes
}
