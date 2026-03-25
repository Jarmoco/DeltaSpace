/* -----------------------------------------------------------------------------
 * Interactive Diff Explorer
 *
 * Provides a ncurses-style TUI for navigating filesystem changes between
 * two snapshots. Uses a virtual directory tree with cursor state, allowing
 * users to drill down into specific changed paths.
 * -------------------------------------------------------------------------- */

use std::{collections::HashMap, io::Write, path::Path};

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

        let help_base = "↑↓ move  → drill  ← back  g toggle chart   q quit";
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
            _ => {
                cursors.insert(parent, cur_idx);
            }
        }
    }
}
