/* -----------------------------------------------------------------------------
 * Interactive Diff Explorer
 *
 * Provides a ncurses-style TUI for navigating filesystem changes between
 * two snapshots. Uses a virtual directory tree with cursor state, allowing
 * users to drill down into specific changed paths.
 * -------------------------------------------------------------------------- */

use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::Path,
};

const CHART_ROWS: usize = 8;
const CHART_PADDING: f64 = 0.20;

fn chart_cols() -> usize {
    crate::terminal::get_width().saturating_sub(20).max(30)
}

fn table_width() -> usize {
    crate::terminal::get_width().saturating_sub(4)
}

fn parse_snapshot_datetime(path: &str) -> Option<(i32, u32, u32, u32, u32)> {
    let name = Path::new(path).file_name()?.to_string_lossy().into_owned();
    let inner = name.strip_prefix("snapshot_")?.strip_suffix(".json")?;
    let parts: Vec<&str> = inner.split('_').collect();
    if parts.len() < 2 {
        return None;
    }
    let date_parts: Vec<&str> = parts[0].split('-').collect();
    let time_parts: Vec<&str> = parts[1].split('-').collect();
    if date_parts.len() < 3 || time_parts.len() < 2 {
        return None;
    }
    Some((
        date_parts[0].parse().ok()?,
        date_parts[1].parse().ok()?,
        date_parts[2].parse().ok()?,
        time_parts[0].parse().ok()?,
        time_parts[1].parse().ok()?,
    ))
}

fn children<'a>(
    diff: &'a HashMap<String, (i64, u64)>,
    parent: Option<&str>,
) -> Vec<(&'a str, i64, u64)> {
    let prefix = match parent {
        Some(p) => format!("{}/", p),
        None => "/".to_string(),
    };
    let mut out: Vec<(&'a str, i64, u64)> = diff
        .iter()
        .filter_map(|(path, &(d, cur))| {
            if crate::constants::is_excluded(path) {
                return None;
            }
            let rest = path.strip_prefix(&prefix)?;
            if rest.contains('/') || rest.is_empty() {
                return None;
            }
            Some((path.as_str(), d, cur))
        })
        .collect();
    out.sort_by(|a, b| b.1.abs().cmp(&a.1.abs()));
    out
}

fn folder_size_over_time(snapshots: &[HashMap<String, u64>], path: &str) -> Vec<u64> {
    let path_key = if path.is_empty() || path == "/" {
        "/".to_string()
    } else {
        path.to_string()
    };
    snapshots
        .iter()
        .map(|snap| *snap.get(&path_key).unwrap_or(&0))
        .collect()
}

fn interpolate(sizes: &[u64], cols: usize) -> Vec<f64> {
    let n = sizes.len();
    if n == 0 {
        return vec![0.0; cols];
    }
    if n == 1 {
        let v = sizes[0] as f64;
        return vec![v; cols];
    }

    let mut result = vec![0.0; cols];
    for col in 0..cols {
        let t = col as f64 / (cols - 1) as f64;
        let pos = t * (n - 1) as f64;
        let lo = pos.floor() as usize;
        let hi = (lo + 1).min(n - 1);
        let frac = pos - pos.floor();
        result[col] = sizes[lo] as f64 * (1.0 - frac) + sizes[hi] as f64 * frac;
    }
    result
}

fn render_chart(
    values: &[f64],
    sizes: &[u64],
    _path: &str,
    dates: &[Option<(i32, u32, u32, u32, u32)>],
) {
    let n = values.len();
    if n == 0 || values.iter().all(|&v| v == 0.0) {
        return;
    }

    let min_v = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_v = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = max_v - min_v;

    let (y_min, y_max) = if range == 0.0 {
        (0.0, max_v * (1.0 + CHART_PADDING))
    } else {
        (
            (min_v - range * CHART_PADDING).max(0.0),
            max_v + range * CHART_PADDING,
        )
    };
    let y_range = y_max - y_min;

    let heights: Vec<usize> = values
        .iter()
        .map(|&v| {
            if y_range == 0.0 {
                1
            } else {
                ((v - y_min) / y_range * (CHART_ROWS as f64)).round() as usize
            }
        })
        .collect();

    let n_snaps = sizes.len();
    let snap_cols: Vec<usize> = (0..n_snaps)
        .map(|i| {
            if n_snaps == 1 {
                chart_cols() / 2
            } else {
                i * (chart_cols() - 1) / (n_snaps - 1)
            }
        })
        .collect();

    let green_grad: Vec<&str> = vec![
        "\x1b[92m█",
        "\x1b[32m▓",
        "\x1b[2;32m▒",
        "\x1b[2;32m▒",
        "\x1b[2;32m▒",
        "\x1b[2;32m░",
        "\x1b[2;32m░",
        "\x1b[2;32m░",
        "\x1b[2;32m░",
    ];
    let red_grad: Vec<&str> = vec![
        "\x1b[91m█",
        "\x1b[31m▓",
        "\x1b[2;31m▒",
        "\x1b[2;31m▒",
        "\x1b[2;31m▒",
        "\x1b[2;31m░",
        "\x1b[2;31m░",
        "\x1b[2;31m░",
        "\x1b[2;31m░",
    ];

    let height_of = |s: u64| -> usize {
        if y_range == 0.0 {
            1
        } else {
            ((s as f64 - y_min) / y_range * (CHART_ROWS as f64)).round() as usize
        }
    };

    let top_label = sizes
        .iter()
        .find(|&&s| height_of(s) >= CHART_ROWS.saturating_sub(1))
        .map(|&s| crate::terminal::fmt_size(s as f64))
        .unwrap_or_default();
    let bot_label = sizes
        .iter()
        .find(|&&s| height_of(s) == 0)
        .map(|&s| crate::terminal::fmt_size(s as f64))
        .unwrap_or_default();

    for row in (0..CHART_ROWS).rev() {
        let label = if row == CHART_ROWS - 1 {
            top_label.clone()
        } else if row == 0 {
            bot_label.clone()
        } else {
            sizes
                .iter()
                .find(|&&s| height_of(s) == row)
                .map(|&s| crate::terminal::fmt_size(s as f64))
                .unwrap_or_default()
        };
        print!("  {:>8} ", label);
        for col in 0..chart_cols().min(n) {
            let h = heights[col].max(1).min(CHART_ROWS);
            if row < h {
                let from_top = h - 1 - row;
                let grad = if col == 0 || n == 1 || values[col] >= values[col.saturating_sub(1)] {
                    &green_grad
                } else {
                    &red_grad
                };
                let gi = (from_top * grad.len() / h).min(grad.len() - 1);
                print!("{}\x1b[0m", grad[gi]);
            } else if snap_cols.contains(&col) {
                print!("\x1b[2;90m│\x1b[0m");
            } else {
                print!(" ");
            }
        }
        println!();
    }

    if n_snaps > 0 {
        let w = chart_cols().min(n);
        let mut date_buf = vec![' '; w];
        let mut time_buf = vec![' '; w];
        for (i, &col) in snap_cols.iter().enumerate() {
            if col >= w {
                break;
            }
            if let Some(d) = dates[i] {
                let date_str = format!("{:02}-{:02}", d.2, d.1);
                let time_str = format!("{:02}:{:02}", d.3, d.4);
                let ds = if col + date_str.len() > w {
                    w - date_str.len()
                } else {
                    col
                };
                let ts = if col + time_str.len() > w {
                    w - time_str.len()
                } else {
                    col
                };
                for (j, ch) in date_str.chars().enumerate() {
                    date_buf[ds + j] = ch;
                }
                for (j, ch) in time_str.chars().enumerate() {
                    time_buf[ts + j] = ch;
                }
            }
        }
        print!("  {:>8} ", "");
        for &ch in &date_buf {
            print!("{}", ch);
        }
        println!();
        print!("  {:>8} ", "");
        for &ch in &time_buf {
            print!("{}", ch);
        }
        println!();
    }
}

/* --- main command --- */

pub fn cmd_explore(idx_a: usize, idx_b: usize) {
    let files = crate::snapshot::cmd_list(false);
    crate::utils::check_indices(&files, &[idx_a, idx_b]);
    let diff = crate::snapshot::build_diff(&files[idx_a], &files[idx_b]);
    if diff.is_empty() {
        println!("No differences found.");
        return;
    }

    let mut snapshots: Vec<HashMap<String, u64>> = Vec::new();
    let mut snapshot_dates: Vec<Option<(i32, u32, u32, u32, u32)>> = Vec::new();
    for i in idx_a..=idx_b {
        snapshots.push(crate::snapshot::load_flat(&files[i]));
        snapshot_dates.push(parse_snapshot_datetime(&files[i]));
    }

    let mut stack: Vec<Option<String>> = vec![None];
    let mut cursors: HashMap<Option<String>, usize> = HashMap::new();
    let mut chart_visible = true;

    loop {
        let parent = stack.last().unwrap().clone();
        let parent_str = parent.as_deref().unwrap_or("");
        let rows = children(&diff, parent.as_deref());
        let max_idx = rows.len().saturating_sub(1);
        let cur_idx = cursors.get(&parent).copied().unwrap_or(0).min(max_idx);

        let available_height = crate::terminal::get_height();
        let base_lines = 7;
        let chart_lines = CHART_ROWS + 2 + 2;
        let required_with_chart = base_lines + rows.len() + chart_lines;
        let show_chart = chart_visible && required_with_chart <= available_height;

        crate::terminal::clear();
        println!();
        println!("  PATH : {}", parent_str);
        println!("  {:<14}  {:<12}  NAME", "CHANGE", "CURRENT");
        println!("  {}", "─".repeat(table_width()));

        if rows.is_empty() {
            println!("  (no changed sub-folders)");
        }

        let total_change = rows.iter().map(|(_, d, _)| d).sum::<i64>();

        for (i, (path, d, cur)) in rows.iter().enumerate() {
            let name = Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let sign = if *d > 0 { '+' } else { '-' };
            let color = if *d > 0 { "\x1b[92m" } else { "\x1b[91m" };
            let reset = "\x1b[0m";
            let sel = if i == cur_idx { "\x1b[7m" } else { "" };
            println!(
                "  {}{}{}{:<13}{}  {:<12}  {}{}",
                sel,
                color,
                sign,
                crate::terminal::fmt_size(d.unsigned_abs() as f64),
                reset,
                crate::terminal::fmt_size(*cur as f64),
                name,
                reset,
            );
        }

        println!("  {}", "─".repeat(table_width()));

        if parent.is_none() {
            let color = if total_change > 0 {
                "\x1b[92m"
            } else {
                "\x1b[91m"
            };
            let reset = "\x1b[0m";
            println!(
                "  {}Total change:                 {}{}",
                color,
                crate::terminal::fmt_size(total_change as f64),
                reset
            );
            println!("  {}", "─".repeat(table_width()));
        }

        if show_chart {
            let chart_path = if !rows.is_empty() {
                rows[cur_idx].0
            } else {
                parent_str
            };
            let size_over_time = folder_size_over_time(&snapshots, chart_path);
            let interpolated = interpolate(&size_over_time, chart_cols());
            render_chart(&interpolated, &size_over_time, chart_path, &snapshot_dates);
            println!("  {}", "─".repeat(table_width()));
        }

        let help_base = "↑↓ move  → drill  ← back  g toggle chart   d delete   q quit";
        println!("  {help_base}");

        let _ = std::io::stdout().flush();

        match crate::terminal::getch().as_str() {
            "q" | "Q" | "\x03" => break,
            "\x1b[A" | "k" => {
                cursors.insert(parent, cur_idx.saturating_sub(1));
            }
            "\x1b[B" | "j" => {
                cursors.insert(parent, (cur_idx + 1).min(max_idx));
            }
            "\x1b[C" | "\r" | "\n" | "l" if !rows.is_empty() => {
                cursors.insert(parent.clone(), cur_idx);
                stack.push(Some(rows[cur_idx].0.to_string()));
            }
            "\x1b[D" | "b" | "h" | "\x7f" if stack.len() > 1 => {
                cursors.insert(parent, cur_idx);
                stack.pop();
            }
            "g" | "G" => {
                chart_visible = !chart_visible;
            }
            "d" | "D" if !rows.is_empty() => {
                let files = crate::snapshot::cmd_list(false);
                if idx_b != files.len() - 1 {
                    println!(
                        "\n  \x1b[93mCan only delete when comparing to the latest snapshot.\x1b[0m"
                    );
                    println!(
                        "  Currently viewing snapshot {} of {}.",
                        idx_b + 1,
                        files.len()
                    );
                    crate::utils::pause();
                    continue;
                }

                let selected_path = rows[cur_idx].0;
                let target_dir = crate::constants::TARGET_DIR.trim_end_matches('/');
                let full_path = format!("{}/{}", target_dir, selected_path.trim_start_matches('/'));

                crate::terminal::clear();
                println!();
                println!("  \x1b[1mDelete folder?\x1b[0m");
                println!("  {}", "─".repeat(table_width()));
                println!("  Path: {}", full_path);
                println!("  {}", "─".repeat(table_width()));
                println!("  Press Enter 3 times to confirm deletion:");
                let _ = io::stdout().flush();

                let mut presses = 0usize;
                while presses < 3 {
                    print!("\r  [");
                    for i in 0..3 {
                        if i > 0 {
                            print!(" ");
                        }
                        if i < presses {
                            let color = match i {
                                0 => "\x1b[32m",
                                1 => "\x1b[33m",
                                _ => "\x1b[31m",
                            };
                            print!("{}██████████\x1b[0m", color);
                        } else {
                            print!("\x1b[90m██████████\x1b[0m");
                        }
                    }
                    print!("]");
                    let _ = io::stdout().flush();

                    match crate::terminal::getch().as_str() {
                        "\r" | "\n" => {
                            presses += 1;
                        }
                        _ => {
                            println!("\n  Cancelled.");
                            crate::utils::pause();
                            break;
                        }
                    }
                }

                if presses == 3 {
                    print!("\r  [");
                    for i in 0..3 {
                        if i > 0 {
                            print!(" ");
                        }
                        let color = match i {
                            0 => "\x1b[32m",
                            1 => "\x1b[33m",
                            _ => "\x1b[31m",
                        };
                        print!("{}██████████\x1b[0m", color);
                    }
                    println!("]");
                    let _ = io::stdout().flush();

                    let result = fs::remove_dir_all(&full_path);
                    let mut succeeded = result.is_ok();

                    if let Err(err) = result {
                        if err.kind() == std::io::ErrorKind::PermissionDenied {
                            println!("  \x1b[93mPermission denied, trying with elevated privileges...\x1b[0m");
                            let _ = io::stdout().flush();

                            let sudo_result = std::process::Command::new("pkexec")
                                .args(["rm", "-rf", &full_path])
                                .output();

                            match sudo_result {
                                Ok(output) if output.status.success() => {
                                    println!(
                                        "\n  Deleted (with elevated privileges): {}",
                                        full_path
                                    );
                                    succeeded = true;
                                }
                                Ok(output) => {
                                    let stderr = String::from_utf8_lossy(&output.stderr);
                                    eprintln!("\n  Failed to delete (elevated): {}", stderr.trim());
                                }
                                Err(e) => {
                                    eprintln!("\n  Failed to run elevated command: {}", e);
                                }
                            }
                        } else {
                            eprintln!("\n  Failed to delete: {}", err);
                        }
                    }

                    if succeeded {
                        println!("\n  Creating new snapshot with deletion applied...");
                        let _ = io::stdout().flush();
                        match crate::snapshot::apply_deletion(idx_b, selected_path) {
                            Ok(new_path) => {
                                println!("\n  Created: {}", new_path);
                            }
                            Err(e) => {
                                eprintln!("\n  Failed to create snapshot: {}", e);
                            }
                        }
                    }
                    crate::utils::pause();
                    break;
                }
            }
            _ => {
                cursors.insert(parent, cur_idx);
            }
        }
    }
}
