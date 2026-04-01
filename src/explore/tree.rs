/* -----------------------------------------------------------------------------
 * explore/tree.rs
 * Builds and renders a virtual directory tree from a diff map with cursor
 * state, scrolling, and path navigation.
 * -------------------------------------------------------------------------- */

use std::{collections::HashMap, path::Path};

/* --- Constants --- */

pub fn table_width() -> usize {
    crate::terminal::get_width().saturating_sub(4)
}

/* --- Helpers --- */

pub fn parse_snapshot_datetime(path: &str) -> Option<(i32, u32, u32, u32, u32)> {
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

pub fn children<'a>(
    diff: &'a HashMap<String, (i64, u64)>,
    parent: Option<&str>,
) -> Vec<(&'a str, i64, u64)> {
    let prefix = match parent {
        Some(p) => format!("{}/", p),
        None => "/".to_string(),
    };
    let mut out: Vec<(&'a str, i64, u64)> = diff
        .iter()
        .filter_map(|(path, &(d, current_size))| {
            if crate::constants::is_excluded(path) {
                return None;
            }
            let rest = path.strip_prefix(&prefix)?;
            if rest.contains('/') || rest.is_empty() {
                return None;
            }
            Some((path.as_str(), d, current_size))
        })
        .collect();
    out.sort_by(|a, b| b.1.abs().cmp(&a.1.abs()));
    out
}

/* --- Scroll --- */

pub fn compute_scroll_offset(
    scroll_offsets: &HashMap<Option<String>, usize>,
    parent: &Option<String>,
    max_idx: usize,
) -> usize {
    scroll_offsets
        .get(parent)
        .copied()
        .unwrap_or(0)
        .min(max_idx)
}

pub fn compute_visible_rows(
    rows_len: usize,
    scroll_offset: usize,
    available_rows: usize,
) -> (bool, bool, usize) {
    let has_more_above = scroll_offset > 0;
    let has_more_below = scroll_offset + available_rows < rows_len;
    let indicator_rows = (has_more_above as usize) + (has_more_below as usize);
    let data_rows = available_rows.saturating_sub(indicator_rows);
    (has_more_above, has_more_below, data_rows)
}

/* --- Rendering --- */

pub fn render_table_rows(
    rows: &[(&str, i64, u64)],
    scroll_offset: usize,
    data_rows: usize,
    cursor_index: usize,
    pending_deletions: &[String],
    has_more_above: bool,
    has_more_below: bool,
) {
    if rows.is_empty() {
        println!("  (no changed sub-folders)");
        return;
    }

    if has_more_above {
        println!("  \x1b[90m↑ scroll up for more --\x1b[0m");
    }

    let visible_rows: Vec<_> = rows
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(data_rows)
        .collect();

    for (i, (path, d, current_size)) in visible_rows {
        let name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let sel = if i == cursor_index { "\x1b[7m" } else { "" };
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
            crate::terminal::fmt_size(*current_size as f64),
            name,
            reset,
        );
    }

    if has_more_below {
        println!("  \x1b[90m-- scroll down for more ↓\x1b[0m");
    }
}
