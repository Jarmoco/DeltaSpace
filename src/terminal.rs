/* -----------------------------------------------------------------------------
 * Terminal I/O & TTY Control
 *
 * Provides low-level terminal operations for the interactive UI.
 * Includes raw TTY mode for single-key input (getch), screen clearing,
 * and human-readable size formatting.
 * -------------------------------------------------------------------------- */

use std::{
    fs,
    io::{self, Read, Write},
    process::Command,
};

pub fn tty_raw() {
    Command::new("stty")
        .args(["-F", "/dev/tty", "raw", "-echo", "min", "1", "time", "0"])
        .status()
        .ok();
}

pub fn tty_raw_timeout() {
    Command::new("stty")
        .args(["-F", "/dev/tty", "raw", "-echo", "min", "0", "time", "1"])
        .status()
        .ok();
}

pub fn tty_restore() {
    Command::new("stty")
        .args(["-F", "/dev/tty", "sane"])
        .status()
        .ok();
}

pub fn getch() -> String {
    let mut tty = match fs::OpenOptions::new().read(true).open("/dev/tty") {
        Ok(f) => f,
        Err(_) => return String::new(),
    };

    tty_raw();

    let mut first = [0u8; 1];
    let mut result = Vec::with_capacity(4);

    if tty.read(&mut first).unwrap_or(0) > 0 {
        result.push(first[0]);

        if first[0] == 0x1b {
            tty_raw_timeout();
            let mut seq = [0u8; 3];
            let n = tty.read(&mut seq).unwrap_or(0);
            if n > 0 {
                result.extend_from_slice(&seq[..n]);
            }
        }
    }

    tty_restore();
    String::from_utf8_lossy(&result).into_owned()
}

pub fn clear() {
    print!("\x1b[H\x1b[J");
    let _ = io::stdout().flush();
}

pub fn fmt_size(mut n: f64) -> String {
    for unit in &["B", "KB", "MB", "GB", "TB"] {
        if n.abs() < 1024.0 {
            return format!("{:.1} {}", n, unit);
        }
        n /= 1024.0;
    }
    format!("{:.1} PB", n)
}
