/* -----------------------------------------------------------------------------
 * Snapshot Pruner
 *
 * Interactive TUI for selecting and deleting old snapshots. Groups
 * snapshots by year/month/week for easier navigation, with toggle/dselect-
 * all functionality and confirmation dialog before deletion.
 * -------------------------------------------------------------------------- */

use std::{
    fs,
    io::{self, Write},
    path::Path,
};

/* --- parsing helpers --- */

pub fn parse_snapshot_date(path: &str) -> Option<(i32, u32, u32)> {
    let name = Path::new(path).file_name()?.to_string_lossy().into_owned();
    let inner = name.strip_prefix("snapshot_")?.strip_suffix(".json")?;
    let date = inner.split('_').next()?;
    let p: Vec<&str> = date.split('-').collect();
    if p.len() < 3 {
        return None;
    }
    Some((p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?))
}

fn week_of_month(day: u32) -> u32 {
    (day - 1) / 7 + 1
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "?",
    }
}

/* --- entry model --- */

#[derive(Clone)]
pub struct SnapEntry {
    pub path: String,
    pub name: String,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub marked: bool,
}

pub fn group_snapshots(files: &[String]) -> Vec<SnapEntry> {
    let mut entries: Vec<SnapEntry> = files
        .iter()
        .filter_map(|f| {
            let (y, mo, d) = parse_snapshot_date(f)?;
            Some(SnapEntry {
                path: f.clone(),
                name: Path::new(f)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                year: y,
                month: mo,
                day: d,
                marked: false,
            })
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

/* --- TUI rendering --- */

pub fn render_prune(entries: &[SnapEntry], cursor: usize) {
    crate::terminal::clear();
    println!();
    println!("  PRUNE SNAPSHOTS");
    println!("  ↑↓/jk navigate   Space/x toggle   a all   n none   d DELETE marked   q back");
    println!("  {}", "─".repeat(70));

    let mut last_ym: Option<(i32, u32)> = None;
    let mut last_week: Option<u32> = None;

    for (i, e) in entries.iter().enumerate() {
        let ym = (e.year, e.month);
        let week = week_of_month(e.day);

        if Some(ym) != last_ym {
            if last_ym.is_some() {
                println!();
            }
            let heading = format!("── {} {:04} ", month_name(e.month), e.year);
            let fill = "─".repeat(70usize.saturating_sub(4 + heading.len()));
            println!("  \x1b[1m{}{}──\x1b[0m", heading, fill);
            last_ym = Some(ym);
            last_week = None;
        }

        if Some(week) != last_week {
            println!("    \x1b[2mWeek {}\x1b[0m", week);
            last_week = Some(week);
        }

        let check = if e.marked {
            "\x1b[91m[x]\x1b[0m"
        } else {
            "[ ]"
        };
        let sel = if i == cursor { "\x1b[7m" } else { "" };
        let reset = "\x1b[0m";
        println!("      {}{} {}{}", sel, check, e.name, reset);
    }

    println!();
    println!("  {}", "─".repeat(70));

    let n_marked = entries.iter().filter(|e| e.marked).count();
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

/* --- main command --- */

pub fn cmd_prune() {
    let files = crate::snapshot::cmd_list(false);
    if files.is_empty() {
        println!(
            "No snapshots found in {}.",
            crate::constants::get_output_dir()
        );
        crate::utils::pause();
        return;
    }

    let mut entries = group_snapshots(&files);
    let mut cursor = 0usize;

    loop {
        render_prune(&entries, cursor);

        let n = entries.len();
        match crate::terminal::getch().as_str() {
            "\x1b[A" | "k" => {
                cursor = cursor.saturating_sub(1);
            }
            "\x1b[B" | "j" => {
                cursor = (cursor + 1).min(n.saturating_sub(1));
            }

            " " | "x" if !entries.is_empty() => {
                entries[cursor].marked = !entries[cursor].marked;
                cursor = (cursor + 1).min(n.saturating_sub(1));
            }

            "a" => {
                entries.iter_mut().for_each(|e| e.marked = true);
            }
            "n" => {
                entries.iter_mut().for_each(|e| e.marked = false);
            }

            "d" | "D" => {
                let marked_count = entries.iter().filter(|e| e.marked).count();
                if marked_count == 0 {
                    continue;
                }

                crate::terminal::clear();
                println!(
                    "\n  About to permanently DELETE {} snapshot(s):\n",
                    marked_count
                );
                for e in entries.iter().filter(|e| e.marked) {
                    println!("    {}", e.name);
                }
                print!("\n  Type YES to confirm, anything else to cancel: ");
                let _ = io::stdout().flush();

                let mut confirm = String::new();
                io::stdin().read_line(&mut confirm).ok();

                if confirm.trim() == "YES" {
                    let (mut deleted, mut failed) = (0u32, 0u32);
                    for e in entries.iter().filter(|e| e.marked) {
                        match fs::remove_file(&e.path) {
                            Ok(_) => deleted += 1,
                            Err(err) => {
                                eprintln!("  failed {}: {}", e.name, err);
                                failed += 1;
                            }
                        }
                    }
                    println!("\n  Deleted: {}  Failed: {}", deleted, failed);
                    crate::utils::pause();
                    let fresh = crate::snapshot::cmd_list(false);
                    entries = group_snapshots(&fresh);
                    cursor = cursor.min(entries.len().saturating_sub(1));
                } else {
                    println!("  Cancelled.");
                    crate::utils::pause();
                }
            }

            "q" | "Q" | "\x03" | "\x1b[D" | "b" | "h" => break,

            _ => {}
        }
    }
}
