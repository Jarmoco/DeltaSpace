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

    println!();
    println!("  DeltaSpace — Select snapshots to compare");
    if selection_phase == 0 {
        println!("  j/k/arrows: navigate   Enter: select   q: cancel");
    } else {
        println!("  j/k/arrows: navigate   Enter: select   ← back   q: cancel");
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

    let max_visible = terminal::get_height().saturating_sub(8);
    let scroll_offset = if entries.len() > max_visible {
        let active_cursor = if selection_phase == 0 {
            baseline_cursor
        } else {
            comparison_cursor
        };
        if active_cursor >= max_visible {
            active_cursor - max_visible + 1
        } else {
            0
        }
    } else {
        0
    };

    let mut last_day: Option<(i32, u32, u32)> = None;

    let mut i = scroll_offset;
    let end = (scroll_offset + max_visible).min(entries.len());
    while i < end {
        let entry = &entries[i];

        let current_day = (entry.year, entry.month, entry.day);
        if last_day.is_some() && last_day != Some(current_day) {
            println!();
        }
        last_day = Some(current_day);

        let label = prune::format_display_compact(entry);

        let is_left_cursor = selection_phase == 0 && i == baseline_cursor;
        let is_left_selected = selection_phase >= 1 && Some(i) == selected_baseline_index;
        let is_right_cursor = selection_phase == 1 && i == comparison_cursor;

        let left = if is_left_cursor {
            format!("\x1b[7m ▸ {:<w$}\x1b[0m", label, w = CELL_W)
        } else if is_left_selected {
            format!("   {:<w$}", label, w = CELL_W)
        } else if selection_phase >= 1 {
            format!("   \x1b[2m{:<w$}\x1b[0m", label, w = CELL_W)
        } else {
            format!("   {:<w$}", label, w = CELL_W)
        };

        let right = if is_right_cursor {
            format!("\x1b[7m ▸ {:<w$}\x1b[0m", label, w = CELL_W)
        } else if selection_phase == 0 {
            format!("   \x1b[2m{:<w$}\x1b[0m", label, w = CELL_W)
        } else {
            format!("   {:<w$}", label, w = CELL_W)
        };

        println!("  {}{}{}", left, " ".repeat(GAP), right);

        i += 1;
    }

    if entries.len() > max_visible {
        println!();
        println!(
            "  \x1b[2m  Showing {}–{} of {}\x1b[0m",
            scroll_offset + 1,
            entries.len().min(scroll_offset + max_visible),
            entries.len()
        );
    }

    println!();
    let _ = std::io::stdout().flush();
}

/* --- Main ----------------------------------------------------------------- */
pub fn select_snapshot_pair(files: &[String]) -> Option<(usize, usize)> {
    let entries = prune::group_snapshots(files);
    if entries.is_empty() {
        return None;
    }

    let mut baseline_cursor = 0usize;
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
                }
            }
            "\x1b[B" | "j" => {
                if selection_phase == 0 {
                    baseline_cursor = (baseline_cursor + 1).min(n.saturating_sub(1));
                } else {
                    comparison_cursor = (comparison_cursor + 1).min(n.saturating_sub(1));
                }
            }
            "\r" | "\n" => {
                if selection_phase == 0 {
                    selected_baseline_index = Some(baseline_cursor);
                    selection_phase = 1;
                    comparison_cursor = if baseline_cursor + 1 < n {
                        baseline_cursor + 1
                    } else if baseline_cursor > 0 {
                        baseline_cursor - 1
                    } else {
                        0
                    };
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
