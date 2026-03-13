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

use std::{io::Write, path::Path};

fn interactive_menu() {
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
                for (i, f) in files.iter().enumerate() {
                    println!(
                        "  [{}] {}",
                        i,
                        Path::new(f)
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    );
                }
                print!("\n  Baseline index : ");
                let _ = std::io::stdout().flush();
                let a = utils::read_usize_line();
                print!("  Comparison index: ");
                let _ = std::io::stdout().flush();
                let b = utils::read_usize_line();
                match (a, b) {
                    (Some(ai), Some(bi)) if ai < files.len() && bi < files.len() => {
                        explore::cmd_explore(ai, bi);
                    }
                    _ => {
                        println!("Invalid.");
                        utils::pause();
                    }
                }
            }
            "3" => {
                terminal::clear();
                prune::cmd_prune();
            }
            "q" | "Q" | "\x03" => {
                println!("Bye!");
                break;
            }
            _ => {}
        }
    }
}

fn main() {
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
            explore::cmd_explore(a, b);
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
  show  <idx>                 Print a snapshot as flat JSON
  explore <old_idx> <new_idx> Open the interactive diff explorer

Exit codes: 0 success, 1 error, 2 bad arguments"
            );
        }
        unknown => {
            utils::die(&format!("Unknown command '{}'. Try --help", unknown), 2);
        }
    }
}
