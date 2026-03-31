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

pub fn cmd_explore(idx_a: usize, mut idx_b: usize) {
    let mut files = crate::snapshot::cmd_list(false);
    crate::utils::check_indices(&files, &[idx_a, idx_b]);

    let mut pending_deletions: Vec<String> = Vec::new();

    loop {
        let diff = crate::snapshot::build_diff(&files[idx_a], &files[idx_b]);
        if diff.is_empty() {
            println!("No differences found.");
            break;
        }

        let mut snapshots: Vec<HashMap<String, u64>> = Vec::new();
        let mut snapshot_dates: Vec<Option<(i32, u32, u32, u32, u32)>> = Vec::new();
        for i in idx_a..=idx_b {
            snapshots.push(crate::snapshot::load_flat(&files[i]));
            snapshot_dates.push(parse_snapshot_datetime(&files[i]));
        }

        let mut stack: Vec<Option<String>> = vec![None];
        let mut cursors: HashMap<Option<String>, usize> = HashMap::new();
        let mut scroll_offsets: HashMap<Option<String>, usize> = HashMap::new();
        let mut chart_visible = true;

        loop {
            let parent = stack.last().unwrap().clone();
            let parent_str = parent.as_deref().unwrap_or("");
            let rows = children(&diff, parent.as_deref());
            let max_idx = rows.len().saturating_sub(1);
            let cur_idx = cursors.get(&parent).copied().unwrap_or(0).min(max_idx);

            let terminal_height = crate::terminal::get_height();
            let header_lines = 4;
            let separator_after_rows = 1;
            let total_change_lines = if parent.is_none() { 2 } else { 0 };
            let chart_block_lines = CHART_ROWS + 2;
            let chart_separator = 1;
            let help_lines = 1;

            let available_rows_no_chart = terminal_height
                .saturating_sub(
                    header_lines + separator_after_rows + total_change_lines + help_lines,
                )
                .max(1);
            let rows_count = rows.len().max(1);
            let actual_rows = available_rows_no_chart.min(rows_count);

            let min_chart_lines = chart_block_lines + chart_separator + help_lines;
            let min_table_lines =
                header_lines + separator_after_rows + actual_rows + total_change_lines + help_lines;
            let show_chart = chart_visible && terminal_height >= min_table_lines + min_chart_lines;

            let bottom_lines = if show_chart {
                min_chart_lines
            } else {
                help_lines
            };
            let available_rows = terminal_height
                .saturating_sub(
                    header_lines + separator_after_rows + total_change_lines + bottom_lines,
                )
                .max(1);

            let scroll_offset = scroll_offsets
                .get(&parent)
                .copied()
                .unwrap_or(0)
                .min(max_idx);

            let has_more_above = scroll_offset > 0;
            let has_more_below = scroll_offset + available_rows < rows.len();
            let indicator_rows = (has_more_above as usize) + (has_more_below as usize);
            let data_rows = available_rows.saturating_sub(indicator_rows);
            let scroll_offset = scroll_offset.min(rows.len().saturating_sub(data_rows));

            crate::terminal::clear();
            println!();
            println!("  PATH : {}", parent_str);
            println!("  {:<14}  {:<12}  NAME", "CHANGE", "CURRENT");
            println!("  {}", "─".repeat(table_width()));

            let total_change = rows.iter().map(|(_, d, _)| d).sum::<i64>();
            let has_more_above = scroll_offset > 0;
            let has_more_below = scroll_offset + available_rows < rows.len();

            if rows.is_empty() {
                println!("  (no changed sub-folders)");
            } else {
                if has_more_above {
                    println!("  \x1b[90m↑ scroll up for more --\x1b[0m");
                }

                let indicator_rows = (has_more_above as usize) + (has_more_below as usize);
                let data_rows = available_rows.saturating_sub(indicator_rows);

                let visible_rows: Vec<_> = rows
                    .iter()
                    .enumerate()
                    .skip(scroll_offset)
                    .take(data_rows)
                    .collect();

                for (i, (path, d, cur)) in visible_rows {
                    let name = Path::new(path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let sel = if i == cur_idx { "\x1b[7m" } else { "" };
                    let reset = "\x1b[0m";

                    let is_queued = pending_deletions.iter().any(|p| p == path);
                    let color = if is_queued {
                        "\x1b[93m"
                    } else if *d > 0 {
                        "\x1b[92m"
                    } else {
                        "\x1b[91m"
                    };
                    let sign_out = if is_queued {
                        '!'
                    } else if *d > 0 {
                        '+'
                    } else {
                        '-'
                    };

                    println!(
                        "  {}{}{}{:<13}{}  {:<12}  {}{}",
                        sel,
                        color,
                        sign_out,
                        crate::terminal::fmt_size(d.unsigned_abs() as f64),
                        reset,
                        crate::terminal::fmt_size(*cur as f64),
                        name,
                        reset,
                    );
                }

                if has_more_below {
                    println!("  \x1b[90m-- scroll down for more ↓\x1b[0m");
                }
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
                let lines_used = header_lines
                    + separator_after_rows
                    + rows
                        .iter()
                        .enumerate()
                        .skip(scroll_offset)
                        .take(available_rows)
                        .count()
                    + total_change_lines
                    + if rows.is_empty() { 1 } else { 0 };
                let filler = terminal_height.saturating_sub(lines_used + bottom_lines);
                for _ in 0..filler {
                    println!();
                }

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

            let pending_count = pending_deletions.len();
            let (help_base, help_extra): (String, Option<String>) = if !show_chart && chart_visible
            {
                let base = if pending_count > 0 {
                    format!("↑↓ move  → drill  ← back  g toggle chart   d: queue delete   x: delete({pending_count})   q quit")
                } else {
                    "↑↓ move  → drill  ← back  g toggle chart   d: queue delete   q quit"
                        .to_string()
                };
                (base, Some("Terminal too small for the chart".to_string()))
            } else if pending_count > 0 {
                (
                    format!("↑↓ move  → drill  ← back  g toggle chart   d: queue delete   x: delete({pending_count})   q quit"),
                    None,
                )
            } else {
                (
                    "↑↓ move  → drill  ← back  g toggle chart   d: queue delete   q quit"
                        .to_string(),
                    None,
                )
            };
            println!("  {help_base}");
            if let Some(extra) = help_extra {
                println!("  \x1b[90m{}\x1b[0m", extra);
            }

            let _ = std::io::stdout().flush();

            match crate::terminal::getch().as_str() {
                "q" | "Q" | "\x03" => {
                    return;
                }
                "\x1b[A" | "k" => {
                    let new_cur = cur_idx.saturating_sub(1);
                    cursors.insert(parent.clone(), new_cur);
                    if new_cur < scroll_offset {
                        scroll_offsets.insert(parent, new_cur);
                    }
                }
                "\x1b[B" | "j" => {
                    let new_cur = (cur_idx + 1).min(max_idx);
                    cursors.insert(parent.clone(), new_cur);

                    // Calculate data_rows assuming we're scrolled (has_more_above = true)
                    let after_more_below = scroll_offset + available_rows < rows.len();
                    let after_indicator = 1 + after_more_below as usize; // has_more_above always true after first scroll
                    let after_data = available_rows.saturating_sub(after_indicator);

                    if new_cur >= scroll_offset + after_data {
                        let new_scroll = new_cur.saturating_sub(after_data) + 1;
                        scroll_offsets.insert(
                            parent,
                            new_scroll.min(rows.len().saturating_sub(after_data)),
                        );
                    }
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
                    let selected_path = rows[cur_idx].0;
                    if !pending_deletions.iter().any(|p| p == selected_path) {
                        pending_deletions.push(selected_path.to_string());
                    }
                }
                "x" | "X" if !pending_deletions.is_empty() => {
                    files = crate::snapshot::cmd_list(false);
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

                    crate::terminal::clear();
                    println!();
                    println!(
                        "  \x1b[1mDelete {} folder(s)?\x1b[0m",
                        pending_deletions.len()
                    );
                    println!("  {}", "─".repeat(table_width()));
                    for (i, p) in pending_deletions.iter().enumerate() {
                        let full_path = format!(
                            "{}/{}",
                            crate::constants::TARGET_DIR.trim_end_matches('/'),
                            p.trim_start_matches('/')
                        );
                        println!("  {}. {}", i + 1, full_path);
                    }
                    println!("  {}", "─".repeat(table_width()));
                    println!("  Press Enter 3 times to confirm deletion:");
                    let _ = io::stdout().flush();

                    let mut presses = 0usize;
                    let mut cancelled = false;
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
                                cancelled = true;
                                break;
                            }
                        }
                    }

                    if cancelled {
                        println!("\n  Cancelled.");
                        crate::utils::pause();
                        continue;
                    }

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

                    let target_dir = crate::constants::TARGET_DIR.trim_end_matches('/');
                    let mut all_succeeded = true;
                    for path in &pending_deletions {
                        let full_path = format!("{}/{}", target_dir, path.trim_start_matches('/'));
                        let result = fs::remove_dir_all(&full_path);
                        if let Err(err) = result {
                            if err.kind() == std::io::ErrorKind::PermissionDenied {
                                println!("  \x1b[93mPermission denied for {}, trying with elevated privileges...\x1b[0m", full_path);
                                let _ = io::stdout().flush();
                                let sudo_result = std::process::Command::new("pkexec")
                                    .args(["rm", "-rf", &full_path])
                                    .output();
                                match sudo_result {
                                    Ok(output) if output.status.success() => {
                                        println!("  Deleted (elevated): {}", full_path);
                                    }
                                    Ok(output) => {
                                        let stderr = String::from_utf8_lossy(&output.stderr);
                                        eprintln!(
                                            "\n  Failed to delete (elevated): {}",
                                            stderr.trim()
                                        );
                                        all_succeeded = false;
                                    }
                                    Err(e) => {
                                        eprintln!("\n  Failed to run elevated command: {}", e);
                                        all_succeeded = false;
                                    }
                                }
                            } else {
                                eprintln!("\n  Failed to delete {}: {}", full_path, err);
                                all_succeeded = false;
                            }
                        }
                    }

                    if all_succeeded {
                        println!("\n  Creating new snapshot with deletions applied...");
                        let _ = io::stdout().flush();
                        match crate::snapshot::apply_deletions(idx_b, &pending_deletions) {
                            Ok(new_path) => {
                                println!("\n  Created: {}", new_path);
                                files = crate::snapshot::cmd_list(false);
                                idx_b = files.len() - 1;
                                pending_deletions.clear();
                                crate::utils::pause();
                                break;
                            }
                            Err(e) => {
                                eprintln!("\n  Failed to create snapshot: {}", e);
                                crate::utils::pause();
                            }
                        }
                    } else {
                        crate::utils::pause();
                    }
                }
                _ => {
                    cursors.insert(parent, cur_idx);
                }
            }
        }
    }
}
