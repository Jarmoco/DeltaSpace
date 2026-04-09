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

#[cfg(target_os = "windows")]
mod windows;

mod signal;

/* --- Constants ------------------------------------------------------------ */

pub static ALTERNATE_MODE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(not(target_os = "windows"))]
pub use signal::init_signal_handler;

#[cfg(target_os = "windows")]
pub fn init_signal_handler() {
    // Windows: Ctrl+C handling is done via SetConsoleCtrlHandler
    // For minimal implementation, we rely on the panic hook in main.rs
    // which handles cleanup on abnormal termination
}

/* --- FFI (Unix-only) ------------------------------------------------------ */

#[cfg(not(target_os = "windows"))]
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

#[cfg(not(target_os = "windows"))]
const STDIN_FILENO: i32 = 0;

#[cfg(not(target_os = "windows"))]
const STDOUT_FILENO: i32 = 1;

#[cfg(not(target_os = "windows"))]
unsafe extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
}

/* --- Size Queries --------------------------------------------------------- */

#[cfg(not(target_os = "windows"))]
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

#[cfg(not(target_os = "windows"))]
pub fn get_width() -> usize {
    get_terminal_size_impl(STDOUT_FILENO)
        .or_else(|| get_terminal_size_impl(STDIN_FILENO))
        .map(|(w, _)| w)
        .unwrap_or(80)
}

#[cfg(not(target_os = "windows"))]
pub fn get_height() -> usize {
    get_terminal_size_impl(STDOUT_FILENO)
        .or_else(|| get_terminal_size_impl(STDIN_FILENO))
        .map(|(_, h)| h)
        .unwrap_or(24)
}

#[cfg(target_os = "windows")]
pub fn get_width() -> usize {
    windows::get_terminal_size().map(|(w, _)| w).unwrap_or(80)
}

#[cfg(target_os = "windows")]
pub fn get_height() -> usize {
    windows::get_terminal_size().map(|(_, h)| h).unwrap_or(24)
}

pub fn init_terminal_size() {
    let _ = get_width();
    let _ = get_height();
}

/* --- TTY Control ---------------------------------------------------------- */

#[cfg(not(target_os = "windows"))]
pub fn tty_fd() -> Option<std::fs::File> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()
}

#[cfg(target_os = "windows")]
pub fn tty_fd() -> Option<std::fs::File> {
    // On Windows, use CONIN$ for console input
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("CON$")
        .ok()
}

#[cfg(target_os = "linux")]
pub use linux::{tty_raw, tty_raw_timeout, tty_restore};

#[cfg(target_os = "macos")]
pub use macos::{tty_raw, tty_raw_timeout, tty_restore};

#[cfg(target_os = "windows")]
pub use windows::{tty_raw, tty_raw_timeout, tty_restore};

/* --- Input ---------------------------------------------------------------- */

#[cfg(not(target_os = "windows"))]
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

#[cfg(target_os = "windows")]
pub fn getch() -> String {
    use std::os::windows::io::AsRawHandle;

    let stdin = std::io::stdin();
    let handle = stdin.as_raw_handle();

    tty_raw();

    let mut first = [0u8; 1];
    let mut result = Vec::with_capacity(4);

    unsafe {
        let mut bytes_read: u32 = 0;
        let success = windows::ReadFile(
            handle as *mut std::ffi::c_void,
            first.as_mut_ptr(),
            1,
            &mut bytes_read,
            std::ptr::null_mut(),
        );

        if success != 0 && bytes_read > 0 {
            result.push(first[0]);

            if first[0] == 0xe0 {
                // Windows uses 0xe0 prefix for special keys
                tty_raw_timeout();
                let mut seq = [0u8; 3];
                let mut n: u32 = 0;
                windows::ReadFile(
                    handle as *mut std::ffi::c_void,
                    seq.as_mut_ptr(),
                    3,
                    &mut n,
                    std::ptr::null_mut(),
                );
                if n > 0 {
                    result.extend_from_slice(&seq[..n as usize]);
                }
            }
        }
    }

    tty_restore();

    // Convert Windows arrow keys to ANSI sequences
    match result.as_slice() {
        [0xe0, 0x48] => "\x1b[A".to_string(), // Up
        [0xe0, 0x50] => "\x1b[B".to_string(), // Down
        [0xe0, 0x4d] => "\x1b[C".to_string(), // Right
        [0xe0, 0x4b] => "\x1b[D".to_string(), // Left
        _ => String::from_utf8_lossy(&result).into_owned(),
    }
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

/* --- Windows Re-exports --------------------------------------------------- */

#[cfg(target_os = "windows")]
pub use windows::ReadFile;
