/* -----------------------------------------------------------------------------
 * prune/mod.rs
 * Interactive command for selecting and deleting old snapshots. Groups
 * snapshots by date, handles user input for toggling entries, and executes
 * deletions with confirmation.
 * -------------------------------------------------------------------------- */

mod model;
mod render;
mod smart;

use std::{
    fs,
    io::{self, Write},
};

use model::format_display_name;
pub use model::{format_display_compact, group_snapshots, SnapEntry};
use render::render_prune;

/* --- Confirmation --- */

pub(crate) fn run_deletion_confirmation(entries: &[SnapEntry]) -> bool {
    let marked: Vec<&SnapEntry> = entries.iter().filter(|e| e.marked).collect();
    let marked_count = marked.len();

    crate::terminal::clear();
    println!(
        "\n  About to permanently DELETE {} snapshot(s):\n",
        marked_count
    );
    for entry in &marked {
        println!("    {}", format_display_name(entry));
    }
    println!("\n  Press Enter 3 times to confirm deletion:");
    let _ = io::stdout().flush();

    let mut presses = 0usize;
    while presses < 3 {
        print!("\r  [");
        for i in 0..3 {
            if i > 0 {
                print!(" ");
            }
            if i < presses {
                let color = match i {
                    0 => "\x1b[32m",
                    1 => "\x1b[33m",
                    _ => "\x1b[31m",
                };
                print!("{}██████████\x1b[0m", color);
            } else {
                print!("\x1b[90m██████████\x1b[0m");
            }
        }
        print!("]");
        let _ = io::stdout().flush();

        match crate::terminal::getch().as_str() {
            "\r" | "\n" => {
                presses += 1;
            }
            _ => {
                println!("\n  Cancelled.");
                crate::utils::pause();
                return false;
            }
        }
    }

    print!("\r  [");
    for i in 0..3 {
        if i > 0 {
            print!(" ");
        }
        let color = match i {
            0 => "\x1b[32m",
            1 => "\x1b[33m",
            _ => "\x1b[31m",
        };
        print!("{}██████████\x1b[0m", color);
    }
    println!("]");
    let _ = io::stdout().flush();

    true
}

/* --- Execution --- */

pub(crate) fn execute_deletions(entries: &mut [SnapEntry]) -> (u32, u32) {
    let (mut deleted, mut failed) = (0u32, 0u32);
    for entry in entries.iter_mut().filter(|e| e.marked) {
        match fs::remove_file(&entry.path) {
            Ok(_) => deleted += 1,
            Err(err) => {
                eprintln!("  failed {}: {}", entry.name, err);
                failed += 1;
            }
        }
    }
    println!("\n  Deleted: {}  Failed: {}", deleted, failed);
    crate::utils::pause();
    (deleted, failed)
}

/* --- Constants ------------------------------------------------------------- */

const PRUNE_METHODS: &[&str] = &[
    "Smart pruning (recommended)",
    "Manual pruning",
];

/* --- Main ----------------------------------------------------------------- */

pub fn cmd_prune() {
    let has_snapshots = !crate::snapshot::cmd_list(false).is_empty();
    if !has_snapshots {
        println!(
            "No snapshots found in {}.",
            crate::constants::get_output_dir()
        );
        crate::utils::pause();
        return;
    }

    let mut cursor = 0usize;
    const ITEMS: usize = 3;

    loop {
        crate::terminal::clear();
        println!();
        println!("  Select your pruning method:");
        println!();
        let mut i = 0usize;
        while i < ITEMS {
            let label = if i == 2 {
                "[q] Go back".to_string()
            } else {
                format!("[{}] {}", i + 1, PRUNE_METHODS[i])
            };
            if i == cursor {
                println!("  \x1b[7m▸\x1b[0m\x1b[7m{}\x1b[0m", label);
            } else {
                println!("   {}", label);
            }
            i += 1;
        }
        println!();
        println!("  \x1b[2m\u{2191}\u{2193}/j/k: navigate   Enter: select   1-2/q: quick select\x1b[0m");
        println!();
        let _ = io::stdout().flush();

        match crate::terminal::getch().as_str() {
            "\x1b[A" | "k" => {
                cursor = cursor.saturating_sub(1);
            }
            "\x1b[B" | "j" => {
                cursor = (cursor + 1).min(ITEMS - 1);
            }
            "\r" | "\n" => match cursor {
                0 => smart::run_smart_mode(),
                1 => cmd_prune_manual(),
                _ => break,
            },
            "1" => {
                smart::run_smart_mode();
            }
            "2" => {
                cmd_prune_manual();
            }
            "q" | "Q" | "\x03" => break,
            _ => {}
        }
    }
}

fn cmd_prune_manual() {
    let files = crate::snapshot::cmd_list(false);
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

            " " | "x" | "\r" | "\n" if !entries.is_empty() => {
                entries[cursor].marked = !entries[cursor].marked;
                cursor = (cursor + 1).min(n.saturating_sub(1));
            }

            "a" => {
                for entry in entries.iter_mut() {
                    entry.marked = true;
                }
            }
            "n" => {
                for entry in entries.iter_mut() {
                    entry.marked = false;
                }
            }

            "d" | "D" => {
                let marked_count = entries.iter().filter(|e| e.marked).count();
                if marked_count == 0 {
                    continue;
                }

                if !run_deletion_confirmation(&entries) {
                    continue;
                }

                execute_deletions(&mut entries);
                let fresh = crate::snapshot::cmd_list(false);
                entries = group_snapshots(&fresh);
                cursor = cursor.min(entries.len().saturating_sub(1));
            }

            "q" | "Q" | "\x03" | "\x1b[D" | "b" | "h" => break,

            _ => {}
        }
    }
}

/* --- CLI helpers ----------------------------------------------------------- */

pub fn list_smart_methods() {
    println!("Available pruning methods:\n");
    for method in smart::SmartMethod::all() {
        println!("  {:<20} {}", method.name(), method.description());
    }
}

pub fn run_smart_cli(method_name: &str) {
    let method = match smart::SmartMethod::from_name(method_name) {
        Some(m) => m,
        None => {
            eprintln!(
                "Unknown pruning method '{}'. \
                 Use 'deltaspace prune list' to see available methods.",
                method_name
            );
            std::process::exit(2);
        }
    };

    let files = crate::snapshot::cmd_list(false);
    if files.is_empty() {
        println!("No snapshots found.");
        return;
    }

    let mut entries = group_snapshots(&files);
    smart::apply_smart_strategy(&mut entries, method);

    let marked_count = entries.iter().filter(|e| e.marked).count();
    if marked_count == 0 {
        println!("No snapshots to delete with method '{}'.", method_name);
        return;
    }

    let (mut deleted, mut failed) = (0u32, 0u32);
    for entry in entries.iter().filter(|e| e.marked) {
        match fs::remove_file(&entry.path) {
            Ok(_) => {
                println!("  deleted  {}", entry.name);
                deleted += 1;
            }
            Err(err) => {
                eprintln!("  failed   {}: {}", entry.name, err);
                failed += 1;
            }
        }
    }
    println!("\nDeleted: {}  Failed: {}", deleted, failed);
}
