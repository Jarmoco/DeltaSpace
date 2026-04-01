/* -----------------------------------------------------------------------------
 * explore/chart.rs
 * Renders a size-over-time bar chart with gradient colors and date axis.
 * -------------------------------------------------------------------------- */

/* --- Constants --- */

pub const CHART_ROWS: usize = 8;
pub const CHART_PADDING: f64 = 0.20;

/* --- Helpers --- */

pub fn chart_cols() -> usize {
    crate::terminal::get_width().saturating_sub(20).max(30)
}

pub fn folder_size_over_time(
    snapshots: &[std::collections::HashMap<String, u64>],
    path: &str,
) -> Vec<u64> {
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

pub fn interpolate(sizes: &[u64], cols: usize) -> Vec<f64> {
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

/* --- Rendering --- */

pub fn render_chart(
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
