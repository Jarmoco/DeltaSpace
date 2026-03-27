use crate::prune;
use crate::terminal;
use std::io::Write;

pub fn render_selector(
    entries: &[prune::SnapEntry],
    cursor_left: usize,
    cursor_right: usize,
    stage: u8,
    selected_base: Option<usize>,
) {
    terminal::clear();
    let gap = 6usize;
    let cell_w: usize = 22;
    let col_w = cell_w + 3;

    println!();
    println!("  DeltaSpace — Select snapshots to compare");
    if stage == 0 {
        println!("  j/k/arrows: navigate   Enter: select   q: cancel");
    } else {
        println!("  j/k/arrows: navigate   Enter: select   ← back   q: cancel");
    }
    println!();
    println!(
        "  \x1b[1m{:<w$}\x1b[0m{}\x1b[1mCOMPARISON\x1b[0m",
        "BASELINE",
        " ".repeat(gap),
        w = col_w
    );
    println!(
        "  \x1b[90m{}\x1b[0m{}\x1b[90m{}\x1b[0m",
        "─".repeat(col_w),
        " ".repeat(gap),
        "─".repeat(col_w)
    );

    let max_visible = terminal::get_height().saturating_sub(8);
    let scroll_offset = if entries.len() > max_visible {
        let active_cursor = if stage == 0 {
            cursor_left
        } else {
            cursor_right
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

    for (i, e) in entries
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(max_visible)
    {
        let current_day = (e.year, e.month, e.day);
        if last_day.is_some() && last_day != Some(current_day) {
            println!();
        }
        last_day = Some(current_day);

        let label = prune::format_display_compact(e);

        let is_left_cursor = stage == 0 && i == cursor_left;
        let is_left_selected = stage >= 1 && Some(i) == selected_base;
        let is_right_cursor = stage == 1 && i == cursor_right;

        let left = if is_left_cursor {
            format!("\x1b[7m ▸ {:<w$}\x1b[0m", label, w = cell_w)
        } else if is_left_selected {
            format!("   {:<w$}", label, w = cell_w)
        } else if stage >= 1 {
            format!("   \x1b[2m{:<w$}\x1b[0m", label, w = cell_w)
        } else {
            format!("   {:<w$}", label, w = cell_w)
        };

        let right = if is_right_cursor {
            format!("\x1b[7m ▸ {:<w$}\x1b[0m", label, w = cell_w)
        } else if stage == 0 {
            format!("   \x1b[2m{:<w$}\x1b[0m", label, w = cell_w)
        } else {
            format!("   {:<w$}", label, w = cell_w)
        };

        println!("  {}{}{}", left, " ".repeat(gap), right);
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

pub fn select_snapshot_pair(files: &[String]) -> Option<(usize, usize)> {
    let entries = prune::group_snapshots(files);
    if entries.is_empty() {
        return None;
    }

    let mut cursor_left = 0usize;
    let mut cursor_right = 0usize;
    let mut stage: u8 = 0;
    let mut selected_base: Option<usize> = None;
    let n = entries.len();

    loop {
        render_selector(&entries, cursor_left, cursor_right, stage, selected_base);

        match terminal::getch().as_str() {
            "\x1b[A" | "k" => {
                if stage == 0 {
                    cursor_left = cursor_left.saturating_sub(1);
                } else {
                    cursor_right = cursor_right.saturating_sub(1);
                }
            }
            "\x1b[B" | "j" => {
                if stage == 0 {
                    cursor_left = (cursor_left + 1).min(n.saturating_sub(1));
                } else {
                    cursor_right = (cursor_right + 1).min(n.saturating_sub(1));
                }
            }
            "\r" | "\n" => {
                if stage == 0 {
                    selected_base = Some(cursor_left);
                    stage = 1;
                    cursor_right = if cursor_left + 1 < n {
                        cursor_left + 1
                    } else if cursor_left > 0 {
                        cursor_left - 1
                    } else {
                        0
                    };
                } else {
                    let bi = selected_base.unwrap();
                    let ci = cursor_right;
                    if bi == ci {
                        continue;
                    }
                    let (base_idx, comp_idx) = if bi < ci { (bi, ci) } else { (ci, bi) };
                    return Some((base_idx, comp_idx));
                }
            }
            "\x1b[D" | "h" => {
                if stage == 1 {
                    stage = 0;
                    selected_base = None;
                }
            }
            "q" | "Q" | "\x1b" => {
                return None;
            }
            _ => {}
        }
    }
}
