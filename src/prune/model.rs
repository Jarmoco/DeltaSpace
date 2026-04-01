/* -----------------------------------------------------------------------------
 * prune/model.rs
 * Defines the SnapEntry data model, snapshot date parsing, grouping logic,
 * and display formatting helpers.
 * -------------------------------------------------------------------------- */

use std::path::Path;

/* --- Parsing --- */

pub fn parse_snapshot_date(path: &str) -> Option<(i32, u32, u32, u32, u32)> {
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

pub fn week_of_month(day: u32) -> u32 {
    (day - 1) / 7 + 1
}

pub fn month_name(m: u32) -> &'static str {
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

/* --- Grouping --- */

#[derive(Clone)]
pub struct SnapEntry {
    pub path: String,
    pub name: String,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub marked: bool,
}

pub fn group_snapshots(files: &[String]) -> Vec<SnapEntry> {
    let mut entries: Vec<SnapEntry> = files
        .iter()
        .filter_map(|file| {
            let (year, month, day, hour, minute) = parse_snapshot_date(file)?;
            Some(SnapEntry {
                path: file.clone(),
                name: Path::new(file)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                year,
                month,
                day,
                hour,
                minute,
                marked: false,
            })
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

/* --- Formatting --- */

pub fn format_display_name(entry: &SnapEntry) -> String {
    format!(
        "Snapshot: \x1b[94m{:02}\x1b[0m \x1b[90m{}\x1b[0m \x1b[2m{:04}\x1b[0m - \x1b[32m{:02}\x1b[90m:\x1b[32m{:02}\x1b[0m",
        entry.day,
        month_name(entry.month),
        entry.year,
        entry.hour,
        entry.minute,
    )
}

pub fn format_display_name_plain(entry: &SnapEntry) -> String {
    format!(
        "Snapshot: {:02} {} {:04} - {:02}:{:02}",
        entry.day,
        month_name(entry.month),
        entry.year,
        entry.hour,
        entry.minute,
    )
}

pub fn format_display_compact(entry: &SnapEntry) -> String {
    format!(
        "{:02} {} {:04}  {:02}:{:02}",
        entry.day,
        month_name(entry.month),
        entry.year,
        entry.hour,
        entry.minute,
    )
}
