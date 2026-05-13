/* -----------------------------------------------------------------------------
 * prune/smart.rs
 * Smart pruning strategies and interactive mode selection.
 * -------------------------------------------------------------------------- */

use std::io::{self, Write};

use super::{group_snapshots, SnapEntry};
use super::model::week_of_month;

/* --- Types ----------------------------------------------------------------- */

pub enum SmartMethod {
    DayFirstLast,
    DayLast,
    DayFirst,
    WeekFirstLast,
    WeekLast,
    WeekFirst,
}

impl SmartMethod {
    pub fn all() -> &'static [SmartMethod] {
        &[
            SmartMethod::DayFirstLast,
            SmartMethod::DayLast,
            SmartMethod::DayFirst,
            SmartMethod::WeekFirstLast,
            SmartMethod::WeekLast,
            SmartMethod::WeekFirst,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            SmartMethod::DayFirstLast => "day-first-last",
            SmartMethod::DayLast => "day-last",
            SmartMethod::DayFirst => "day-first",
            SmartMethod::WeekFirstLast => "week-first-last",
            SmartMethod::WeekLast => "week-last",
            SmartMethod::WeekFirst => "week-first",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            SmartMethod::DayFirstLast => "Day-First-Last",
            SmartMethod::DayLast => "Day-Last",
            SmartMethod::DayFirst => "Day-First",
            SmartMethod::WeekFirstLast => "Week-First-Last",
            SmartMethod::WeekLast => "Week-Last",
            SmartMethod::WeekFirst => "Week-First",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SmartMethod::DayFirstLast => "Keep first and last snapshot per day",
            SmartMethod::DayLast => "Keep only the last snapshot of each day",
            SmartMethod::DayFirst => "Keep only the first snapshot of each day",
            SmartMethod::WeekFirstLast => "Keep snapshots of first and last day of each week",
            SmartMethod::WeekLast => "Keep only the last snapshot of each week",
            SmartMethod::WeekFirst => "Keep only the first snapshot of each week",
        }
    }

    pub fn from_name(s: &str) -> Option<SmartMethod> {
        match s {
            "day-first-last" => Some(SmartMethod::DayFirstLast),
            "day-last" => Some(SmartMethod::DayLast),
            "day-first" => Some(SmartMethod::DayFirst),
            "week-first-last" => Some(SmartMethod::WeekFirstLast),
            "week-last" => Some(SmartMethod::WeekLast),
            "week-first" => Some(SmartMethod::WeekFirst),
            _ => None,
        }
    }
}

/* --- Strategy -------------------------------------------------------------- */

pub fn apply_smart_strategy(entries: &mut [SnapEntry], method: SmartMethod) {
    for entry in entries.iter_mut() {
        entry.marked = false;
    }

    if entries.is_empty() {
        return;
    }

    let use_week = matches!(
        method,
        SmartMethod::WeekFirstLast | SmartMethod::WeekLast | SmartMethod::WeekFirst
    );

    let mut group_start = 0usize;
    for i in 1..=entries.len() {
        let last_entry = i == entries.len();
        let different_group = !last_entry
            && if use_week {
                entries[i].year != entries[i - 1].year
                    || entries[i].month != entries[i - 1].month
                    || week_of_month(
                        entries[i].year,
                        entries[i].month,
                        entries[i].day,
                    ) != week_of_month(
                        entries[i - 1].year,
                        entries[i - 1].month,
                        entries[i - 1].day,
                    )
            } else {
                entries[i].year != entries[i - 1].year
                    || entries[i].month != entries[i - 1].month
                    || entries[i].day != entries[i - 1].day
            };

        if last_entry || different_group {
            let group_end = i - 1;
            match method {
                SmartMethod::DayFirstLast | SmartMethod::WeekFirstLast => {
                    if group_end - group_start >= 1 {
                        for j in group_start + 1..group_end {
                            entries[j].marked = true;
                        }
                    }
                }
                SmartMethod::DayLast | SmartMethod::WeekLast => {
                    for j in group_start..group_end {
                        entries[j].marked = true;
                    }
                }
                SmartMethod::DayFirst | SmartMethod::WeekFirst => {
                    for j in group_start + 1..=group_end {
                        entries[j].marked = true;
                    }
                }
            }
            group_start = i;
        }
    }
}

/* --- Interactive menu ------------------------------------------------------ */

fn select_method() -> Option<SmartMethod> {
    let mut cursor = 0usize;
    loop {
        crate::terminal::clear();
        println!();
        println!("  Smart Pruning");
        println!("  \x1b[2m\u{2191}\u{2193}/jk navigate  Enter select  q back\x1b[0m");
        println!();
        let methods = SmartMethod::all();
        for (i, method) in methods.iter().enumerate() {
            let label = format!(
                "[{}] {}  \u{2014} {}",
                i + 1,
                method.display_name(),
                method.description()
            );
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
                cursor = (cursor + 1).min(methods.len() - 1);
            }
            "1" => return Some(SmartMethod::DayFirstLast),
            "2" => return Some(SmartMethod::DayLast),
            "3" => return Some(SmartMethod::DayFirst),
            "4" => return Some(SmartMethod::WeekFirstLast),
            "5" => return Some(SmartMethod::WeekLast),
            "6" => return Some(SmartMethod::WeekFirst),
            "\r" | "\n" => {
                return match cursor {
                    0 => Some(SmartMethod::DayFirstLast),
                    1 => Some(SmartMethod::DayLast),
                    2 => Some(SmartMethod::DayFirst),
                    3 => Some(SmartMethod::WeekFirstLast),
                    4 => Some(SmartMethod::WeekLast),
                    5 => Some(SmartMethod::WeekFirst),
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
