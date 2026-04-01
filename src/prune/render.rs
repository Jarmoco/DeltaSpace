/* -----------------------------------------------------------------------------
 * prune/render.rs
 * Renders the prune TUI with grouped snapshot entries, cursor highlighting,
 * and marked-for-deletion status.
 * -------------------------------------------------------------------------- */

use std::io::{self, Write};

use super::model::{
    format_display_name, format_display_name_plain, month_name, week_of_month, SnapEntry,
};

/* --- Rendering --- */

pub fn render_prune(entries: &[SnapEntry], cursor: usize) {
    crate::terminal::clear();
    let w = crate::terminal::get_width();
    println!();
    println!("  PRUNE SNAPSHOTS");
    println!("  ↑↓/jk navigate   Space/x/Enter toggle   a all   n none   d DELETE marked   q back");
    println!("  {}", "─".repeat(w.saturating_sub(4)));

    let mut last_ym: Option<(i32, u32)> = None;
    let mut last_week: Option<u32> = None;
    let mut last_day: Option<u32> = None;

    for (i, entry) in entries.iter().enumerate() {
        let ym = (entry.year, entry.month);
        let week = week_of_month(entry.day);

        if Some(ym) != last_ym {
            if last_ym.is_some() {
                println!();
            }
            let label = format!("{} {:04}", month_name(entry.month), entry.year);
            let fill = "─".repeat(42usize.saturating_sub(5 + label.len()));
            println!(
                "  \x1b[90m──\x1b[0m \x1b[2m{}\x1b[0m \x1b[90m{}\x1b[0m",
                label, fill
            );
            last_ym = Some(ym);
            last_week = None;
            last_day = None;
        }

        if Some(week) != last_week {
            println!("    \x1b[2mWeek {}\x1b[0m", week);
            last_week = Some(week);
            last_day = None;
        }

        if last_day.is_some() && last_day != Some(entry.day) {
            println!();
        }
        last_day = Some(entry.day);

        let check = if entry.marked { "[x]" } else { "[ ]" };
        if i == cursor {
            println!(
                "      \x1b[7m{} {}\x1b[0m",
                check,
                format_display_name_plain(entry)
            );
        } else {
            let marked = if entry.marked { "\x1b[91m" } else { "" };
            let rst = if entry.marked { "\x1b[0m" } else { "" };
            println!(
                "      {}{} {}{}",
                marked,
                check,
                format_display_name(entry),
                rst
            );
        }
    }

    println!();
    println!("  {}", "─".repeat(w.saturating_sub(4)));

    let n_marked = entries.iter().filter(|entry| entry.marked).count();
    if n_marked == 0 {
        println!("  No snapshots marked.");
    } else {
        println!(
            "  \x1b[91m{} snapshot(s) marked for deletion.\x1b[0m",
            n_marked
        );
    }
    let _ = io::stdout().flush();
}
