/* -----------------------------------------------------------------------------
 * prune/render.rs
 * Renders the prune TUI with grouped snapshot entries, cursor highlighting,
 * scrollable output with overflow indicators, and marked-for-deletion status.
 * -------------------------------------------------------------------------- */

use std::io::{self, Write};

use super::model::{
    format_display_name, format_display_name_plain, month_name, week_of_month, SnapEntry,
};

/* --- Rendering --- */

pub fn render_prune(entries: &[SnapEntry], cursor: usize) {
    crate::terminal::clear();
    let w = crate::terminal::get_width();
    let h = crate::terminal::get_height();

    let sep = format!("  {}", "─".repeat(w.saturating_sub(4)));

    /* --- Collect all rendered lines into a Vec --- */

    let mut lines: Vec<String> = Vec::new();
    let mut line_of_entry: Vec<usize> = Vec::new();

    // Header
    lines.push(String::new());
    lines.push("  PRUNE SNAPSHOTS".to_string());
    lines.push(
        "  \x1b[2m\u{2191}\u{2193}/jk navigate   Space/x/Enter toggle   a all   n none   d DELETE marked   q back\x1b[0m"
            .to_string(),
    );
    lines.push(sep.clone());

    // Entries
    let mut last_ym: Option<(i32, u32)> = None;
    let mut last_week: Option<u32> = None;
    let mut last_day: Option<u32> = None;

    for (i, entry) in entries.iter().enumerate() {
        let ym = (entry.year, entry.month);
        let week = week_of_month(entry.year, entry.month, entry.day);

        if Some(ym) != last_ym {
            if last_ym.is_some() {
                lines.push(String::new());
            }
            let label = format!("{} {:04}", month_name(entry.month), entry.year);
            let fill = "─".repeat(42usize.saturating_sub(5 + label.len()));
            lines.push(format!(
                "  \x1b[90m──\x1b[0m \x1b[2m{}\x1b[0m \x1b[90m{}\x1b[0m",
                label, fill
            ));
            last_ym = Some(ym);
            last_week = None;
            last_day = None;
        }

        if Some(week) != last_week {
            lines.push(format!("    \x1b[2mWeek {}\x1b[0m", week));
            last_week = Some(week);
            last_day = None;
        }

        if last_day.is_some() && last_day != Some(entry.day) {
            lines.push(String::new());
        }
        last_day = Some(entry.day);

        line_of_entry.push(lines.len());

        let check = if entry.marked { "[x]" } else { "[ ]" };
        if i == cursor {
            lines.push(format!(
                "      \x1b[7m{} {}\x1b[0m",
                check,
                format_display_name_plain(entry)
            ));
        } else {
            let marked = if entry.marked { "\x1b[91m" } else { "" };
            let rst = if entry.marked { "\x1b[0m" } else { "" };
            lines.push(format!(
                "      {}{} {}{}",
                marked, check, format_display_name(entry), rst
            ));
        }
    }

    // Footer
    lines.push(String::new());
    lines.push(sep);
    let n_marked = entries.iter().filter(|entry| entry.marked).count();
    if n_marked == 0 {
        lines.push("  No snapshots marked.".to_string());
    } else {
        lines.push(format!(
            "  \x1b[91m{} snapshot(s) marked for deletion.\x1b[0m",
            n_marked
        ));
    }

    let total = lines.len();
    const HEADER: usize = 4;
    const FOOTER: usize = 3;

    /* --- Always render the fixed header --- */

    for line in &lines[0..HEADER] {
        println!("{line}");
    }

    /* --- Scroll window over body (entries only) --- */

    let body_total = total.saturating_sub(HEADER + FOOTER);
    let body_avail = h.saturating_sub(HEADER + FOOTER);

    if body_total <= body_avail {
        for line in &lines[HEADER..total - FOOTER] {
            println!("{line}");
        }
        for line in &lines[total - FOOTER..total] {
            println!("{line}");
        }
        let _ = io::stdout().flush();
        return;
    }

    let cursor_line = line_of_entry.get(cursor).copied().unwrap_or(HEADER);
    let cursor_body = cursor_line.saturating_sub(HEADER);

    // Decide which overflow indicators to show based on cursor position
    let third = body_avail / 3;
    let (show_top, show_bot) = if cursor_body <= third {
        (false, true)
    } else if cursor_body >= body_total.saturating_sub(third) {
        (true, false)
    } else {
        (true, true)
    };

    let content =
        body_avail.saturating_sub(show_top as usize + show_bot as usize).max(1);

    // Position cursor at roughly 1/3 from the top of the content window
    let mut top = cursor_body.saturating_sub(content / 3);

    // Ensure room for indicators
    if show_top && top == 0 {
        top = 1;
    }
    if show_bot && top + content >= body_total {
        top = body_total.saturating_sub(content + 1);
    }

    // Nudge if cursor fell out of the window
    if cursor_body < top {
        top = cursor_body;
    } else if cursor_body >= top + content {
        top = (cursor_body + 1).saturating_sub(content);
    }

    // Final clamp
    if show_top && top == 0 {
        top = 1;
    }
    if top + content > body_total {
        top = body_total.saturating_sub(content);
    }

    let bot = top + content;

    /* --- Render body with overflow indicators + fixed footer --- */

    if show_top {
        let n = top;
        let s = if n == 1 { "" } else { "s" };
        println!("  \x1b[2m\u{2191} {} line{} more\x1b[0m", n, s);
    }
    for line in &lines[(HEADER + top)..(HEADER + bot)] {
        println!("{line}");
    }
    if show_bot {
        let n = body_total - bot;
        let s = if n == 1 { "" } else { "s" };
        println!("  \x1b[2m\u{2193} {} line{} more\x1b[0m", n, s);
    }
    for line in &lines[total - FOOTER..total] {
        println!("{line}");
    }

    let _ = io::stdout().flush();
}
