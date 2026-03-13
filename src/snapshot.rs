/* -----------------------------------------------------------------------------
 * Snapshot Management
 *
 * Core domain logic for capturing, loading, and comparing filesystem
 * snapshots. Handles the full lifecycle: scanning → saving as JSON →
 * loading → diffing. Acts as the central coordinator between scan, json,
 * and time modules.
 * -------------------------------------------------------------------------- */

use std::{
    collections::HashMap,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

/* --- commands: capture & list --- */

pub fn cmd_scan(verbose: bool) -> String {
    let output_dir = crate::constants::get_output_dir();
    fs::create_dir_all(&output_dir).unwrap_or_default();
    if verbose {
        println!("Scanning {} …", crate::constants::TARGET_DIR);
    }

    let t0 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let sizes = crate::scan::scan(crate::constants::TARGET_DIR);

    if verbose {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
            - t0;
        println!("\r  {} dirs in {:.1}s{:40}", sizes.len(), elapsed, "");
    }

    let mut layers: HashMap<String, HashMap<String, u64>> = HashMap::new();
    for (path, &size) in &sizes {
        if size >= crate::constants::THRESHOLD && !crate::constants::is_excluded(path) {
            let depth = path.chars().filter(|&c| c == '/').count().to_string();
            layers.entry(depth).or_default().insert(path.clone(), size);
        }
    }

    let value = crate::json_utils::layers_to_json_value(&layers);
    let ts = crate::time::current_timestamp();
    let dest = format!("{}/snapshot_{}.json", output_dir, ts);
    fs::write(&dest, crate::json::stringify(&value)).expect("failed to write snapshot");

    if verbose {
        println!("Saved → {}", dest);
    }
    dest
}

pub fn cmd_list(as_json: bool) -> Vec<String> {
    let output_dir = crate::constants::get_output_dir();
    let mut files: Vec<String> = match fs::read_dir(&output_dir) {
        Ok(rd) => rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with("snapshot_") && name.ends_with(".json") {
                    Some(e.path().to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => vec![],
    };
    files.sort();

    if as_json {
        let arr = crate::json::Value::Array(
            files
                .iter()
                .map(|s| crate::json::Value::String(s.clone()))
                .collect(),
        );
        println!("{}", crate::json::stringify(&arr));
    }
    files
}

/* --- internal: JSON ↔ HashMap --- */

pub fn flatten(data: &crate::json::Value) -> HashMap<String, u64> {
    let mut out = HashMap::new();
    if let crate::json::Value::Object(depth_map) = data {
        for (_depth, paths_val) in depth_map {
            if let crate::json::Value::Object(paths) = paths_val {
                for (path, size_val) in paths {
                    if crate::constants::is_excluded(path) {
                        continue;
                    }
                    if let crate::json::Value::Number(n) = size_val {
                        out.insert(path.clone(), *n as u64);
                    }
                }
            }
        }
    }
    out
}

pub fn load_flat(filepath: &str) -> HashMap<String, u64> {
    let raw = fs::read_to_string(filepath).expect("failed to read snapshot");
    let val = crate::json::parse(&raw).expect("invalid snapshot JSON");
    flatten(&val)
}

pub fn build_diff(file_a: &str, file_b: &str) -> HashMap<String, (i64, u64)> {
    let a = load_flat(file_a);
    let b = load_flat(file_b);
    let mut diff = HashMap::new();
    let all_keys: std::collections::HashSet<&String> = a.keys().chain(b.keys()).collect();
    for path in all_keys {
        let va = *a.get(path).unwrap_or(&0) as i64;
        let vb = *b.get(path).unwrap_or(&0) as i64;
        let d = vb - va;
        if d != 0 {
            diff.insert(path.clone(), (d, *b.get(path).unwrap_or(&0)));
        }
    }
    diff
}

/* --- commands: diff & show --- */

pub fn cmd_diff(idx_a: usize, idx_b: usize, as_json: bool) -> HashMap<String, (i64, u64)> {
    let files = cmd_list(false);
    crate::utils::check_indices(&files, &[idx_a, idx_b]);
    let diff = build_diff(&files[idx_a], &files[idx_b]);
    if as_json {
        println!(
            "{}",
            crate::json::stringify(&crate::json_utils::diff_to_json_value(&diff))
        );
    }
    diff
}

pub fn cmd_show(idx: usize, as_json: bool) -> HashMap<String, u64> {
    let files = cmd_list(false);
    if idx >= files.len() {
        crate::utils::die(
            &format!(
                "Index {} out of range (0–{})",
                idx,
                files.len().saturating_sub(1)
            ),
            2,
        );
    }
    let data = load_flat(&files[idx]);
    if as_json {
        let mut obj = HashMap::new();
        for (k, v) in &data {
            obj.insert(k.clone(), crate::json::Value::Number(*v as f64));
        }
        println!(
            "{}",
            crate::json::stringify(&crate::json::Value::Object(obj))
        );
    }
    data
}
