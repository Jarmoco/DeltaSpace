/* -----------------------------------------------------------------------------
 * Terminal I/O & TTY Control
 *
 * Provides low-level terminal operations for the interactive UI.
 * Includes raw TTY mode for single-key input (getch), screen clearing,
 * and human-readable size formatting.
 * -------------------------------------------------------------------------- */

use std::{
    fs, io,
    io::{Read, Write},
    sync::atomic::Ordering,
};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

mod signal;

/* --- Constants ------------------------------------------------------------ */

pub static ALTERNATE_MODE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub use signal::init_signal_handler;

/* --- FFI ------------------------------------------------------------------ */

#[repr(C)]
struct winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

#[cfg(target_os = "linux")]
const TIOCGWINSZ: u64 = 0x5413;

#[cfg(target_os = "macos")]
const TIOCGWINSZ: u64 = 0x40087468;

const STDIN_FILENO: i32 = 0;
const STDOUT_FILENO: i32 = 1;

unsafe extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
}

/* --- Size Queries --------------------------------------------------------- */

fn get_terminal_size_impl(fd: i32) -> Option<(usize, usize)> {
    unsafe {
        let mut ws = std::mem::zeroed::<winsize>();
        if ioctl(fd, TIOCGWINSZ, &mut ws) == 0 {
            Some((ws.ws_col as usize, ws.ws_row as usize))
        } else {
            None
        }
    }
}

pub fn get_width() -> usize {
    get_terminal_size_impl(STDOUT_FILENO)
        .or_else(|| get_terminal_size_impl(STDIN_FILENO))
        .map(|(w, _)| w)
        .unwrap_or(80)
}

pub fn get_height() -> usize {
    get_terminal_size_impl(STDOUT_FILENO)
        .or_else(|| get_terminal_size_impl(STDIN_FILENO))
        .map(|(_, h)| h)
        .unwrap_or(24)
}

pub fn init_terminal_size() {
    let _ = get_width();
    let _ = get_height();
}

/* --- TTY Control ---------------------------------------------------------- */

pub fn tty_fd() -> Option<std::fs::File> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()
}

#[cfg(target_os = "linux")]
pub use linux::{tty_raw, tty_raw_timeout, tty_restore};

#[cfg(target_os = "macos")]
pub use macos::{tty_raw, tty_raw_timeout, tty_restore};

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

/* --- Screen Control ------------------------------------------------------- */

pub fn clear() {
    print!("\x1b[H\x1b[2J");
    let _ = io::stdout().flush();
}

pub fn enter_alternate_screen() {
    print!("\x1b[?1049h");
    let _ = io::stdout().flush();
    ALTERNATE_MODE.store(true, Ordering::SeqCst);
}

pub fn exit_alternate_screen() {
    print!("\x1b[?1049l");
    let _ = io::stdout().flush();
    ALTERNATE_MODE.store(false, Ordering::SeqCst);
}

/* --- Formatting ----------------------------------------------------------- */

pub fn fmt_size(mut n: f64) -> String {
    for unit in &["B", "KB", "MB", "GB", "TB"] {
        if n.abs() < 1024.0 {
            return format!("{:.1} {}", n, unit);
        }
        n /= 1024.0;
    }
    format!("{:.1} PB", n)
}
