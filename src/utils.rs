/* -----------------------------------------------------------------------------
 * Utilities
 *
 * Small, frequently-used helpers that don't belong in any specific
 * module. Includes console I/O helpers (pause, read input) and fatal error
 * handling (die).
 * -------------------------------------------------------------------------- */

use std::{
    io::{self, Write},
    process,
};

pub fn pause() {
    print!("\n  Enter to continue…");
    let _ = io::stdout().flush();
    let _ = io::stdin().read_line(&mut String::new());
}

pub fn die(msg: &str, code: i32) -> ! {
    eprintln!("error: {}", msg);
    process::exit(code);
}

pub fn check_indices(files: &[String], indices: &[usize]) {
    if files.len() < 2 {
        die(
            &format!(
                "Need ≥ 2 snapshots in {}",
                crate::constants::get_output_dir()
            ),
            1,
        );
    }
    for &i in indices {
        if i >= files.len() {
            die(
                &format!("Index {} out of range (0–{})", i, files.len() - 1),
                2,
            );
        }
    }
}
