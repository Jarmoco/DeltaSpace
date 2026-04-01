/* -----------------------------------------------------------------------------
 * prune/mod.rs
 * Interactive command for selecting and deleting old snapshots. Groups
 * snapshots by date, handles user input for toggling entries, and executes
 * deletions with confirmation.
 * -------------------------------------------------------------------------- */

mod model;
mod render;

use std::{
    fs,
    io::{self, Write},
};

use model::format_display_name;
pub use model::{format_display_compact, group_snapshots, SnapEntry};
use render::render_prune;

/* --- Confirmation --- */

fn run_deletion_confirmation(entries: &[SnapEntry]) -> bool {
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

fn execute_deletions(entries: &mut [SnapEntry]) -> (u32, u32) {
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

/* --- Main --- */

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
