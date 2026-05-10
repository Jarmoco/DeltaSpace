/* -----------------------------------------------------------------------------
 * prune/smart.rs
 * Smart pruning strategies and interactive mode selection.
 * -------------------------------------------------------------------------- */

use std::io::{self, Write};

use super::{group_snapshots, SnapEntry};

/* --- Types ----------------------------------------------------------------- */

pub enum SmartMethod {
    DayLast,
    DayFirst,
    DayFirstLast,
}

/* --- Strategy -------------------------------------------------------------- */

pub fn apply_smart_strategy(entries: &mut [SnapEntry], method: SmartMethod) {
    for entry in entries.iter_mut() {
        entry.marked = false;
    }

    if entries.is_empty() {
        return;
    }

    let mut day_start = 0usize;
    for i in 1..=entries.len() {
        let last_entry = i == entries.len();
        let different_day = !last_entry
            && (entries[i].year != entries[i - 1].year
                || entries[i].month != entries[i - 1].month
                || entries[i].day != entries[i - 1].day);

        if last_entry || different_day {
            let day_end = i - 1;
            match method {
                SmartMethod::DayLast => {
                    for j in day_start..day_end {
                        entries[j].marked = true;
                    }
                }
                SmartMethod::DayFirst => {
                    for j in day_start + 1..=day_end {
                        entries[j].marked = true;
                    }
                }
                SmartMethod::DayFirstLast => {
                    if day_end - day_start >= 1 {
                        for j in day_start + 1..day_end {
                            entries[j].marked = true;
                        }
                    }
                }
            }
            day_start = i;
        }
    }
}

/* --- Interactive menu ------------------------------------------------------ */

fn select_method() -> Option<SmartMethod> {
    let modes: &[(&str, &str)] = &[
        ("Day-Last", "Keep only the last snapshot of each day"),
        ("Day-First", "Keep only the first snapshot of each day"),
        ("Day-First-Last", "Keep first and last snapshot per day"),
    ];

    let mut cursor = 0usize;
    loop {
        crate::terminal::clear();
        println!();
        println!("  Smart Pruning");
        println!("  \x1b[2m\u{2191}\u{2193}/jk navigate  Enter select  q back\x1b[0m");
        println!();
        for (i, (name, desc)) in modes.iter().enumerate() {
            let label = format!("[{}] {}  \u{2014} {}", i + 1, name, desc);
            if i == cursor {
                println!("  \x1b[7m{}\x1b[0m", label);
            } else {
                println!("  {}", label);
            }
        }
        println!();
        let _ = io::stdout().flush();

        match crate::terminal::getch().as_str() {
            "\x1b[A" | "k" => {
                cursor = cursor.saturating_sub(1);
            }
            "\x1b[B" | "j" => {
                cursor = (cursor + 1).min(modes.len() - 1);
            }
            "1" => return Some(SmartMethod::DayLast),
            "2" => return Some(SmartMethod::DayFirst),
            "3" => return Some(SmartMethod::DayFirstLast),
            "\r" | "\n" => {
                return match cursor {
                    0 => Some(SmartMethod::DayLast),
                    1 => Some(SmartMethod::DayFirst),
                    2 => Some(SmartMethod::DayFirstLast),
                    _ => None,
                };
            }
            "q" | "Q" => return None,
            _ => {}
        }
    }
}

/* --- Main interactive entry point ----------------------------------------- */

pub fn run_smart_mode() {
    let files = crate::snapshot::cmd_list(false);
    if files.is_empty() {
        return;
    }

    let method = match select_method() {
        Some(m) => m,
        None => return,
    };

    let mut entries = group_snapshots(&files);
    apply_smart_strategy(&mut entries, method);

    let marked_count = entries.iter().filter(|e| e.marked).count();
    if marked_count == 0 {
        crate::terminal::clear();
        println!("\n  No snapshots to delete with this method.");
        crate::utils::pause();
        return;
    }

    if !super::run_deletion_confirmation(&entries) {
        return;
    }

    super::execute_deletions(&mut entries);
}
