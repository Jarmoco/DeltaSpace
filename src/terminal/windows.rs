/* -----------------------------------------------------------------------------
 * terminal/windows.rs
 * Windows-specific console configuration for raw input mode.
 * Uses Windows Console API via FFI - no external dependencies.
 * -------------------------------------------------------------------------- */

use std::os::windows::io::AsRawHandle;

/* --- Constants ------------------------------------------------------------ */

const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;
const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;

const ENABLE_ECHO_INPUT: u32 = 0x0004;
const ENABLE_LINE_INPUT: u32 = 0x0002;
const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

/* --- FFI ------------------------------------------------------------------ */

#[repr(C)]
pub struct COORD {
    pub x: i16,
    pub y: i16,
}

#[repr(C)]
pub struct SMALL_RECT {
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
}

#[repr(C)]
pub struct CONSOLE_SCREEN_BUFFER_INFO {
    pub size: COORD,
    pub cursor_position: COORD,
    pub attributes: u16,
    pub window: SMALL_RECT,
    pub maximum_window_size: COORD,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
    fn GetConsoleScreenBufferInfo(
        hConsoleOutput: *mut std::ffi::c_void,
        lpConsoleScreenBufferInfo: *mut CONSOLE_SCREEN_BUFFER_INFO,
    ) -> i32;
    fn GetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, lpMode: *mut u32) -> i32;
    fn SetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, dwMode: u32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn ReadFile(
        hFile: *mut std::ffi::c_void,
        lpBuffer: *mut u8,
        nNumberOfBytesToRead: u32,
        lpNumberOfBytesRead: *mut u32,
        lpOverlapped: *mut std::ffi::c_void,
    ) -> i32;
}

/* --- State ---------------------------------------------------------------- */

thread_local! {
    static ORIGINAL_MODE: std::cell::RefCell<Option<u32>> = const { std::cell::RefCell::new(None) };
}

struct Handle(*mut std::ffi::c_void);
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

static STDIN_HANDLE: std::sync::OnceLock<Handle> = std::sync::OnceLock::new();

/* --- Initialization ------------------------------------------------------- */

fn get_stdin() -> *mut std::ffi::c_void {
    STDIN_HANDLE
        .get_or_init(|| unsafe { Handle(GetStdHandle(STD_INPUT_HANDLE)) })
        .0
}

/* --- TTY Control ---------------------------------------------------------- */

pub fn tty_fd() -> Option<std::fs::File> {
    // On Windows, we use stdin/stdout directly
    // Try to open CONIN$ for console input
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("CON$")
        .ok()
}

pub fn tty_raw() {
    let handle = get_stdin();
    if handle.is_null() {
        return;
    }

    unsafe {
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return;
        }

        ORIGINAL_MODE.with(|original| {
            if original.borrow().is_none() {
                *original.borrow_mut() = Some(mode);
            }
        });

        // Disable line input and echo, enable virtual terminal input for ANSI sequences
        let new_mode =
            mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT) | ENABLE_VIRTUAL_TERMINAL_INPUT;
        SetConsoleMode(handle, new_mode);
    }
}

pub fn tty_raw_timeout() {
    let handle = get_stdin();
    if handle.is_null() {
        return;
    }

    unsafe {
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return;
        }

        // Disable line input and echo
        let new_mode = mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT);
        SetConsoleMode(handle, new_mode);
    }
}

pub fn tty_restore() {
    let handle = get_stdin();
    if handle.is_null() {
        return;
    }

    ORIGINAL_MODE.with(|original| {
        if let Some(mode) = *original.borrow() {
            unsafe {
                SetConsoleMode(handle, mode);
            }
        }
    });
}

/* --- Terminal Size -------------------------------------------------------- */

pub fn get_terminal_size() -> Option<(usize, usize)> {
    unsafe {
        let stdout = GetStdHandle(STD_OUTPUT_HANDLE);
        if stdout.is_null() {
            return None;
        }

        let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
        if GetConsoleScreenBufferInfo(stdout, &mut info) == 0 {
            return None;
        }

        let width = (info.window.right - info.window.left + 1) as usize;
        let height = (info.window.bottom - info.window.top + 1) as usize;
        Some((width, height))
    }
}
