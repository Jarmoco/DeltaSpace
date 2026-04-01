/* -----------------------------------------------------------------------------
 * explore/deletion.rs
 * Handles the deletion confirmation flow (triple-enter prompt), executes
 * folder removals with elevated privileges when needed, and creates a new
 * snapshot reflecting the deletions.
 * -------------------------------------------------------------------------- */

use std::{fs, io, io::Write};

/* --- Rendering --- */

pub fn render_deletion_prompt(pending: &[String]) {
    crate::terminal::clear();
    println!();
    println!("  \x1b[1mDelete {} folder(s)?\x1b[0m", pending.len());
    println!("  {}", "─".repeat(super::tree::table_width()));
    for (i, p) in pending.iter().enumerate() {
        let full_path = format!(
            "{}/{}",
            crate::constants::TARGET_DIR.trim_end_matches('/'),
            p.trim_start_matches('/')
        );
        println!("  {}. {}", i + 1, full_path);
    }
    println!("  {}", "─".repeat(super::tree::table_width()));
    println!("  Press Enter 3 times to confirm deletion:");
    let _ = io::stdout().flush();
}

/* --- Confirmation --- */

pub fn run_deletion_confirmation() -> bool {
    let mut presses = 0usize;
    let mut cancelled = false;
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
                cancelled = true;
                break;
            }
        }
    }

    if !cancelled {
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
    }

    !cancelled
}

/* --- Execution --- */

pub fn execute_deletions(pending: &[String]) -> bool {
    let target_dir = crate::constants::TARGET_DIR.trim_end_matches('/');
    let mut all_succeeded = true;
    for path in pending {
        let full_path = format!("{}/{}", target_dir, path.trim_start_matches('/'));
        let result = fs::remove_dir_all(&full_path);
        if let Err(err) = result {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                println!(
                    "  \x1b[93mPermission denied for {}, trying with elevated privileges...\x1b[0m",
                    full_path
                );
                let _ = io::stdout().flush();
                let sudo_result = std::process::Command::new("pkexec")
                    .args(["rm", "-rf", &full_path])
                    .output();
                match sudo_result {
                    Ok(output) if output.status.success() => {
                        println!("  Deleted (elevated): {}", full_path);
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        eprintln!("\n  Failed to delete (elevated): {}", stderr.trim());
                        all_succeeded = false;
                    }
                    Err(e) => {
                        eprintln!("\n  Failed to run elevated command: {}", e);
                        all_succeeded = false;
                    }
                }
            } else {
                eprintln!("\n  Failed to delete {}: {}", full_path, err);
                all_succeeded = false;
            }
        }
    }
    all_succeeded
}

/* --- Snapshot --- */

pub fn apply_and_snapshot(
    comparison_index: &mut usize,
    pending: &[String],
    files: &mut Vec<String>,
) -> bool {
    match crate::snapshot::apply_deletions(*comparison_index, pending) {
        Ok(new_path) => {
            println!("\n  Created: {}", new_path);
            *files = crate::snapshot::cmd_list(false);
            *comparison_index = files.len() - 1;
            true
        }
        Err(e) => {
            eprintln!("\n  Failed to create snapshot: {}", e);
            false
        }
    }
}
