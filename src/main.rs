/* -----------------------------------------------------------------------------
 * deltaspace — filesystem snapshot & diff explorer
 *
 * Usage (interactive):  ./deltaspace
 * Usage (CLI API):      ./deltaspace <command> [args]
 *
 * Commands:
 *   scan                        Scan filesystem and save a snapshot
 *   list                        List available snapshots as JSON
 *   diff  <old_idx> <new_idx>   Print diff between two snapshots as JSON
 *   show  <idx>                 Print a snapshot as flat JSON
 *   explore <old_idx> <new_idx> Open the interactive diff explorer
 *
 * Exit codes: 0 success, 1 error, 2 bad arguments
 * -------------------------------------------------------------------------- */

mod constants;
mod explore;
mod json;
mod json_utils;
mod prune;
mod scan;
mod snapshot;
mod terminal;
mod time;
mod utils;

use std::io::Write;

fn render_selector(
    entries: &[prune::SnapEntry],
    cursor_left: usize,
    cursor_right: usize,
    stage: u8,
    selected_base: Option<usize>,
) {
    terminal::clear();
    let gap = 6usize;
    let cell_w: usize = 22;
    let col_w = cell_w + 3; // "   " prefix + cell content

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

    for (i, e) in entries
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(max_visible)
    {
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

fn select_snapshot_pair(files: &[String]) -> Option<(usize, usize)> {
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

fn interactive_menu() {
    terminal::enter_alternate_screen();

    loop {
        terminal::clear();
        println!();
        println!("  DeltaSpace");
        println!("  [1] Scan filesystem → snapshot");
        println!("  [2] Compare snapshots (interactive)");
        println!("  [3] Prune snapshots");
        println!("  [q] Quit");
        println!();
        let _ = std::io::stdout().flush();

        match terminal::getch().as_str() {
            "1" => {
                terminal::clear();
                snapshot::cmd_scan(true);
                utils::pause();
            }
            "2" => {
                terminal::clear();
                let files = snapshot::cmd_list(false);

                if files.len() < 2 {
                    println!("Need ≥ 2 snapshots in {}.", constants::get_output_dir());
                    utils::pause();
                    continue;
                }

                if let Some((base_idx, comp_idx)) = select_snapshot_pair(&files) {
                    explore::cmd_explore(base_idx, comp_idx);
                }
            }
            "3" => {
                terminal::clear();
                prune::cmd_prune();
            }
            "q" | "Q" | "\x03" => {
                terminal::exit_alternate_screen();
                terminal::tty_restore();
                terminal::clear();
                println!("Bye!");
                println!();
                break;
            }
            _ => {}
        }
    }
}

fn main() {
    std::panic::set_hook(Box::new(|_| {
        terminal::exit_alternate_screen();
        terminal::tty_restore();
        eprintln!("\nTerminated.");
    }));

    terminal::init_signal_handler();
    terminal::init_terminal_size();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        interactive_menu();
        return;
    }

    match args[1].as_str() {
        "scan" => {
            println!("{}", snapshot::cmd_scan(false));
        }
        "list" => {
            snapshot::cmd_list(true);
        }
        "show" => {
            if args.len() < 3 {
                utils::die("Usage: deltaspace show <idx>", 2);
            }
            let idx: usize = args[2]
                .parse()
                .unwrap_or_else(|_| utils::die("idx must be an integer", 2));
            snapshot::cmd_show(idx, true);
        }
        "diff" => {
            if args.len() < 4 {
                utils::die("Usage: deltaspace diff <old_idx> <new_idx>", 2);
            }
            let a: usize = args[2]
                .parse()
                .unwrap_or_else(|_| utils::die("old_idx must be an integer", 2));
            let b: usize = args[3]
                .parse()
                .unwrap_or_else(|_| utils::die("new_idx must be an integer", 2));
            snapshot::cmd_diff(a, b, true);
        }
        "explore" => {
            if args.len() < 4 {
                utils::die("Usage: deltaspace explore <old_idx> <new_idx>", 2);
            }
            let a: usize = args[2]
                .parse()
                .unwrap_or_else(|_| utils::die("old_idx must be an integer", 2));
            let b: usize = args[3]
                .parse()
                .unwrap_or_else(|_| utils::die("new_idx must be an integer", 2));
            // Ensure the baseline index is less than the comparison index
            let (base_idx, comp_idx) = if a < b { (a, b) } else { (b, a) };
            explore::cmd_explore(base_idx, comp_idx);
        }
        "-h" | "--help" => {
            println!(
                "deltaspace — filesystem snapshot & diff explorer

Usage (interactive):  ./deltaspace
Usage (CLI API):      ./deltaspace <command> [args]

Commands:
  scan                        Scan filesystem and save a snapshot
  list                        List available snapshots as JSON
  diff  <old_idx> <new_idx>   Print diff between two snapshots as JSON
                              (old_idx < new_idx is enforced)
  show  <idx>                 Print a snapshot as flat JSON
  explore <old_idx> <new_idx> Open the interactive diff explorer
                              (old_idx < new_idx is enforced)

Exit codes: 0 success, 1 error, 2 bad arguments"
            );
        }
        unknown => {
            utils::die(&format!("Unknown command '{}'. Try --help", unknown), 2);
        }
    }
}
