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

/* --- Path utilities ------------------------------------------------------- */

#[cfg(not(target_os = "windows"))]
const PATH_SEP: &str = "/";

#[cfg(target_os = "windows")]
const PATH_SEP: &str = "\\";

fn join_path_components(components: &[&str]) -> String {
    components.join(PATH_SEP)
}

fn path_starts_with_child(parent: &str, child: &str) -> bool {
    let prefix = format!("{}{}", parent, PATH_SEP);
    child.starts_with(&prefix)
}

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
    let ts = crate::time::get_current_timestamp();
    let dest = join_path_components(&[&output_dir, &format!("snapshot_{}.json", ts)]);
    fs::write(&dest, crate::json::stringify(&value)).expect("failed to write snapshot");

    if verbose {
        println!("Saved → {}", dest);
    }
    dest
}

pub fn cmd_list(as_json: bool) -> Vec<String> {
    let output_dir = crate::constants::get_output_dir();
    let mut files: Vec<String> = match fs::read_dir(&output_dir) {
        Ok(rd) => {
            let mut result = Vec::new();
            for entry in rd {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with("snapshot_") && name.ends_with(".json") {
                    result.push(entry.path().to_string_lossy().into_owned());
                }
            }
            result
        }
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
        let value_a = *a.get(path).unwrap_or(&0) as i64;
        let value_b = *b.get(path).unwrap_or(&0) as i64;
        let d = value_b - value_a;
        if d != 0 {
            diff.insert(path.clone(), (d, *b.get(path).unwrap_or(&0)));
        }
    }
    diff
}

/* --- commands: diff & show --- */

pub fn cmd_diff(
    baseline_index: usize,
    comparison_index: usize,
    as_json: bool,
) -> HashMap<String, (i64, u64)> {
    let files = cmd_list(false);
    crate::utils::check_indices(&files, &[baseline_index, comparison_index]);
    let diff = build_diff(&files[baseline_index], &files[comparison_index]);
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

pub fn apply_deletions(snapshot_idx: usize, paths: &[String]) -> Result<String, String> {
    let files = cmd_list(false);
    if snapshot_idx >= files.len() {
        return Err("Invalid snapshot index".to_string());
    }
    let source_filepath = &files[snapshot_idx];

    let raw = fs::read_to_string(source_filepath).map_err(|e| e.to_string())?;
    let mut val = crate::json::parse(&raw).map_err(|e| e.to_string())?;

    let mut path_sizes: Vec<(String, u64)> = Vec::new();

    if let crate::json::Value::Object(depth_map) = &val {
        for (_depth, paths_val) in depth_map {
            if let crate::json::Value::Object(paths_obj) = paths_val {
                for (p, size_val) in paths_obj {
                    if paths.contains(p) {
                        if let crate::json::Value::Number(n) = size_val {
                            path_sizes.push((p.clone(), *n as u64));
                        }
                    }
                }
            }
        }
    }

    if let crate::json::Value::Object(depth_map) = &mut val {
        for (_depth, paths_val) in depth_map.iter_mut() {
            if let crate::json::Value::Object(paths_obj) = paths_val {
                for (p, size_val) in paths_obj.iter_mut() {
                    let is_target = paths.contains(p);
                    let is_child = paths.iter().any(|target| path_starts_with_child(target, p));
                    if is_target || is_child {
                        if let crate::json::Value::Number(n) = size_val {
                            *n = 0.0;
                        }
                    }
                }
            }
        }

        for (removed_path, removed_size) in &path_sizes {
            for (_, paths_val) in depth_map.iter_mut() {
                if let crate::json::Value::Object(paths_obj) = paths_val {
                    for (p, size_val) in paths_obj.iter_mut() {
                        let is_parent =
                            p != removed_path && path_starts_with_child(p, removed_path);
                        if is_parent {
                            if let crate::json::Value::Number(n) = size_val {
                                let current = *n as i64;
                                let new = (current - *removed_size as i64).max(0);
                                *n = new as f64;
                            }
                        }
                    }
                }
            }
        }
    }

    let output_dir = crate::constants::get_output_dir();
    fs::create_dir_all(&output_dir).unwrap_or_default();
    let ts = crate::time::get_current_timestamp();
    let dest = join_path_components(&[&output_dir, &format!("snapshot_{}.json", ts)]);
    fs::write(&dest, crate::json::stringify(&val)).map_err(|e| e.to_string())?;
    Ok(dest)
}
