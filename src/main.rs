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
mod selector;
mod snapshot;
mod terminal;
mod time;
mod utils;

use std::io::Write;

/* --- Constants -------------------------------------------------------------- */

const MENU_ITEMS: &[&str] = &[
    "Scan filesystem → snapshot",
    "Compare snapshots (interactive)",
    "Prune snapshots",
    "Quit",
];

/* --- CLI parsing ------------------------------------------------------------ */

enum CliCommand {
    Scan,
    List,
    Show {
        idx: usize,
    },
    Diff {
        first_index: usize,
        second_index: usize,
    },
    Explore {
        first_index: usize,
        second_index: usize,
    },
    Help,
}

fn print_help() {
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

fn parse_cli_args(args: &[String]) -> CliCommand {
    match args[1].as_str() {
        "scan" => CliCommand::Scan,
        "list" => CliCommand::List,
        "show" => {
            if args.len() < 3 {
                utils::die("Usage: deltaspace show <idx>", 2);
            }
            let idx: usize = args[2]
                .parse()
                .unwrap_or_else(|_| utils::die("idx must be an integer", 2));
            CliCommand::Show { idx }
        }
        "diff" => {
            if args.len() < 4 {
                utils::die("Usage: deltaspace diff <old_idx> <new_idx>", 2);
            }
            let first_index: usize = args[2]
                .parse()
                .unwrap_or_else(|_| utils::die("old_idx must be an integer", 2));
            let second_index: usize = args[3]
                .parse()
                .unwrap_or_else(|_| utils::die("new_idx must be an integer", 2));
            CliCommand::Diff {
                first_index,
                second_index,
            }
        }
        "explore" => {
            if args.len() < 4 {
                utils::die("Usage: deltaspace explore <old_idx> <new_idx>", 2);
            }
            let first_index: usize = args[2]
                .parse()
                .unwrap_or_else(|_| utils::die("old_idx must be an integer", 2));
            let second_index: usize = args[3]
                .parse()
                .unwrap_or_else(|_| utils::die("new_idx must be an integer", 2));
            CliCommand::Explore {
                first_index,
                second_index,
            }
        }
        "-h" | "--help" => CliCommand::Help,
        unknown => {
            utils::die(&format!("Unknown command '{}'. Try --help", unknown), 2);
        }
    }
}

/* --- Menu rendering --------------------------------------------------------- */

fn render_menu(cursor: usize) {
    terminal::clear();
    println!();
    println!("  DeltaSpace");
    println!();
    let mut i = 0;
    while i < MENU_ITEMS.len() {
        if i == cursor {
            println!("  \x1b[7m▸\x1b[0m\x1b[7m{}\x1b[0m", menu_label(i));
        } else {
            println!("   {}", menu_label(i));
        }
        i += 1;
    }
    println!();
    println!("  \x1b[2m↑↓/j/k: navigate   Enter: select   1-3: quick select   q: quit\x1b[0m");
    println!();
    let _ = std::io::stdout().flush();
}

fn menu_label(index: usize) -> String {
    if index == 3 {
        format!("[q] {}", MENU_ITEMS[index])
    } else {
        format!("[{}] {}", index + 1, MENU_ITEMS[index])
    }
}

/* --- Menu actions ----------------------------------------------------------- */

fn execute_menu_action(index: usize) {
    match index {
        0 => {
            terminal::clear();
            snapshot::cmd_scan(true);
            utils::pause();
        }
        1 => {
            terminal::clear();
            let files = snapshot::cmd_list(false);
            if files.len() < 2 {
                println!("Need ≥ 2 snapshots in {}.", constants::get_output_dir());
                utils::pause();
                return;
            }
            if let Some((base_idx, comp_idx)) = selector::select_snapshot_pair(&files) {
                explore::cmd_explore(base_idx, comp_idx);
            }
        }
        2 => {
            terminal::clear();
            prune::cmd_prune();
        }
        _ => {}
    }
}

/* --- Interactive menu ------------------------------------------------------- */

fn interactive_menu() {
    terminal::enter_alternate_screen();

    let mut menu_cursor: usize = 0;

    loop {
        render_menu(menu_cursor);

        match terminal::getch().as_str() {
            "\x1b[A" | "k" => {
                menu_cursor = menu_cursor.saturating_sub(1);
            }
            "\x1b[B" | "j" => {
                let max = MENU_ITEMS.len() - 1;
                menu_cursor = (menu_cursor + 1).min(max);
            }
            "\r" | "\n" => {
                if menu_cursor == 3 {
                    break;
                }
                execute_menu_action(menu_cursor);
            }
            "1" => {
                execute_menu_action(0);
            }
            "2" => {
                execute_menu_action(1);
            }
            "3" => {
                execute_menu_action(2);
            }
            "q" | "Q" | "\x03" => {
                break;
            }
            _ => {}
        }
    }

    terminal::exit_alternate_screen();
    terminal::tty_restore();
    terminal::clear();
    println!("Bye!");
    println!();
}

/* --- entry point ------------------------------------------------------------ */

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

    match parse_cli_args(&args) {
        CliCommand::Scan => {
            println!("{}", snapshot::cmd_scan(false));
        }
        CliCommand::List => {
            snapshot::cmd_list(true);
        }
        CliCommand::Show { idx } => {
            snapshot::cmd_show(idx, true);
        }
        CliCommand::Diff {
            first_index,
            second_index,
        } => {
            snapshot::cmd_diff(first_index, second_index, true);
        }
        CliCommand::Explore {
            first_index,
            second_index,
        } => {
            let (base_idx, comp_idx) = if first_index < second_index {
                (first_index, second_index)
            } else {
                (second_index, first_index)
            };
            explore::cmd_explore(base_idx, comp_idx);
        }
        CliCommand::Help => {
            print_help();
        }
    }
}
