/* -----------------------------------------------------------------------------
 * Filesystem Scanner
 *
 * Performs a depth-first traversal computing directory sizes from the
 * bottom up. Uses an iterative stack to avoid recursion limits on deep
 * filesystems. Single I/O pass records file sizes and children, then a
 * pure in-memory pass computes sizes in reverse order.
 * -------------------------------------------------------------------------- */

use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::Path,
};

fn normalize_path(path: &str) -> String {
    #[cfg(target_os = "windows")]
    return path.replace('\\', "/");
    #[cfg(not(target_os = "windows"))]
    return path.to_string();
}

/* --- Types ----------------------------------------------------------------- */

struct DirInfo {
    file_total: u64,
    children: Vec<String>,
}

/* --- Main ----------------------------------------------------------------- */

pub fn scan(root: &str) -> HashMap<String, u64> {
    let mut dirs: HashMap<String, DirInfo> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut stack: Vec<String> = vec![normalize_path(root)];
    let mut counter: u64 = 0;

    /* Pass 1 — walk and record (I/O) */
    while let Some(current_path) = stack.pop() {
        if crate::constants::is_excluded(&current_path) {
            continue;
        }
        order.push(current_path.clone());

        let mut file_total: u64 = 0;
        let mut children: Vec<String> = Vec::new();

        if let Ok(rd) = fs::read_dir(&current_path) {
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
                    if !crate::constants::is_excluded(&p) {
                        children.push(p.clone());
                        stack.push(p);
                    }
                } else {
                    file_total += meta.len();
                }
            }
        }

        counter += 1;
        if counter % 200 == 0 {
            let base = Path::new(&current_path)
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            print!(
                "\r  {} dirs — {:<45}",
                counter,
                &base[..base.len().min(45)]
            );
            let _ = io::stdout().flush();
        }

        dirs.insert(
            current_path,
            DirInfo {
                file_total,
                children,
            },
        );
    }

    /* Pass 2 — sum sizes bottom-up (in-memory) */
    let mut sizes: HashMap<String, u64> = HashMap::with_capacity(order.len());
    for path in order.iter().rev() {
        if let Some(info) = dirs.get(path) {
            let mut total = info.file_total;
            for child in &info.children {
                total += sizes.get(child).copied().unwrap_or(0);
            }
            sizes.insert(path.clone(), total);
        }
    }

    sizes
}
