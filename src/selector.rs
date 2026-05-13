/* -----------------------------------------------------------------------------
 * selector.rs
 * Provides a two-column TUI for selecting a baseline and comparison snapshot
 * pair, returning their indices for diff exploration.
 * -------------------------------------------------------------------------- */

use crate::prune;
use crate::terminal;
use std::io::Write;

/* --- Constants ------------------------------------------------------------ */
const GAP: usize = 6;
const CELL_W: usize = 22;
const COL_W: usize = CELL_W + 3;

/* --- Rendering ------------------------------------------------------------ */
pub fn render_selector(
    entries: &[prune::SnapEntry],
    baseline_cursor: usize,
    comparison_cursor: usize,
    selection_phase: u8,
    selected_baseline_index: Option<usize>,
) {
    terminal::clear();
    let h = terminal::get_height();

    let today_ymd = {
        let t = crate::time::get_local_time();
        (t.0 as i32, t.1, t.2)
    };
    let baseline_idx = selected_baseline_index.unwrap_or(usize::MAX);

    /* --- Collect entry lines --- */
    let mut lines: Vec<String> = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        let is_today = (entry.year, entry.month, entry.day) == today_ymd;
        let label = prune::format_display_compact(entry);
        let label_padded = format!("{:<w$}", label, w = CELL_W);

        let is_left_cursor = selection_phase == 0 && i == baseline_cursor;
        let is_left_selected = selection_phase >= 1 && Some(i) == selected_baseline_index;
        let is_right_cursor = selection_phase == 1 && i == comparison_cursor;

        let left = if is_left_cursor {
            format!("\x1b[7m ▸ {}\x1b[0m", label_padded)
        } else if is_left_selected || selection_phase == 0 {
            if is_today {
                format!("\x1b[32m   {}\x1b[0m", label_padded)
            } else {
                format!("   {}", label_padded)
            }
        } else {
            if is_today {
                format!("\x1b[32m\x1b[2m   {}\x1b[0m", label_padded)
            } else {
                format!("\x1b[2m   {}\x1b[0m", label_padded)
            }
        };

        let right = if is_right_cursor {
            format!("\x1b[7m ▸ {}\x1b[0m", label_padded)
        } else if selection_phase == 0 || (selection_phase >= 1 && i <= baseline_idx) {
            if is_today {
                format!("\x1b[32m\x1b[2m   {}\x1b[0m", label_padded)
            } else {
                format!("\x1b[2m   {}\x1b[0m", label_padded)
            }
        } else {
            if is_today {
                format!("\x1b[32m   {}\x1b[0m", label_padded)
            } else {
                format!("   {}", label_padded)
            }
        };

        lines.push(format!("  {}{}{}", left, " ".repeat(GAP), right));
    }

    const HEADER: usize = 6;
    let body_total = lines.len();
    let body_avail = h.saturating_sub(HEADER);

    /* --- Render fixed header --- */
    println!();
    println!("  DeltaSpace — Select snapshots to compare");
    if selection_phase == 0 {
        println!("  \x1b[2mj/k/arrows: navigate   Enter: select   q: cancel\x1b[0m");
    } else {
        println!("  \x1b[2mj/k/arrows: navigate   Enter: select   ← back   q: cancel\x1b[0m");
    }
    println!();
    println!(
        "  \x1b[1m{:<w$}\x1b[0m{}\x1b[1mCOMPARISON\x1b[0m",
        "BASELINE",
        " ".repeat(GAP),
        w = COL_W
    );
    println!(
        "  \x1b[90m{}\x1b[0m{}\x1b[90m{}\x1b[0m",
        "─".repeat(COL_W),
        " ".repeat(GAP),
        "─".repeat(COL_W)
    );

    /* --- Body --- */
    if body_total <= body_avail {
        for line in &lines {
            println!("{line}");
        }
        let _ = std::io::stdout().flush();
        return;
    }

    let active_cursor = if selection_phase == 0 {
        baseline_cursor
    } else {
        comparison_cursor
    };

    let third = body_avail / 3;
    let (show_top, show_bot) = if active_cursor <= third {
        (false, true)
    } else if active_cursor >= body_total.saturating_sub(third) {
        (true, false)
    } else {
        (true, true)
    };

    let content =
        body_avail.saturating_sub(show_top as usize + show_bot as usize).max(1);

    let mut top = active_cursor.saturating_sub(content / 3);

    if show_top && top == 0 {
        top = 1;
    }
    if show_bot && top + content >= body_total {
        top = body_total.saturating_sub(content + 1);
    }

    if active_cursor < top {
        top = active_cursor;
    } else if active_cursor >= top + content {
        top = (active_cursor + 1).saturating_sub(content);
    }

    if show_top && top == 0 {
        top = 1;
    }
    if top + content > body_total {
        top = body_total.saturating_sub(content);
    }

    let bot = top + content;

    if show_top {
        let n = top;
        let s = if n == 1 { "" } else { "s" };
        println!("  \x1b[2m\u{2191} {} line{} more\x1b[0m", n, s);
    }
    for line in &lines[top..bot] {
        println!("{line}");
    }
    if show_bot {
        let n = body_total - bot;
        let s = if n == 1 { "" } else { "s" };
        println!("  \x1b[2m\u{2193} {} line{} more\x1b[0m", n, s);
    }

    let _ = std::io::stdout().flush();
}

/* --- Main ----------------------------------------------------------------- */
pub fn select_snapshot_pair(files: &[String]) -> Option<(usize, usize)> {
    let entries = prune::group_snapshots(files);
    if entries.is_empty() {
        return None;
    }

    let (y, m, d, _, _) = crate::time::get_local_time();
    let today_ymd = (y as i32, m, d);
    let mut baseline_cursor = entries
        .iter()
        .position(|e| (e.year, e.month, e.day) == today_ymd)
        .unwrap_or(0);
    let mut comparison_cursor = 0usize;
    let mut selection_phase: u8 = 0;
    let mut selected_baseline_index: Option<usize> = None;
    let n = entries.len();

    loop {
        render_selector(
            &entries,
            baseline_cursor,
            comparison_cursor,
            selection_phase,
            selected_baseline_index,
        );

        match terminal::getch().as_str() {
            "\x1b[A" | "k" => {
                if selection_phase == 0 {
                    baseline_cursor = baseline_cursor.saturating_sub(1);
                } else {
                    comparison_cursor = comparison_cursor.saturating_sub(1);
                    let min_allowed = selected_baseline_index.unwrap_or(0) + 1;
                    if comparison_cursor < min_allowed {
                        comparison_cursor = min_allowed.min(n.saturating_sub(1));
                    }
                }
            }
            "\x1b[B" | "j" => {
                if selection_phase == 0 {
                    baseline_cursor = (baseline_cursor + 1).min(n.saturating_sub(1));
                } else {
                    comparison_cursor = (comparison_cursor + 1).min(n.saturating_sub(1));
                    let min_allowed = selected_baseline_index.unwrap_or(0) + 1;
                    if comparison_cursor < min_allowed {
                        comparison_cursor = min_allowed.min(n.saturating_sub(1));
                    }
                }
            }
            "\r" | "\n" => {
                if selection_phase == 0 {
                    if baseline_cursor + 1 >= n {
                        continue;
                    }
                    selected_baseline_index = Some(baseline_cursor);
                    selection_phase = 1;
                    comparison_cursor = baseline_cursor + 1;
                } else {
                    let baseline_index = selected_baseline_index.unwrap();
                    let comparison_index = comparison_cursor;
                    if baseline_index == comparison_index {
                        continue;
                    }
                    let (base_idx, comp_idx) = if baseline_index < comparison_index {
                        (baseline_index, comparison_index)
                    } else {
                        (comparison_index, baseline_index)
                    };
                    return Some((base_idx, comp_idx));
                }
            }
            "\x1b[D" | "h" => {
                if selection_phase == 1 {
                    selection_phase = 0;
                    selected_baseline_index = None;
                }
            }
            "q" | "Q" | "\x1b" => {
                return None;
            }
            _ => {}
        }
    }
}
