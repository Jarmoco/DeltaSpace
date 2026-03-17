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
    os::unix::io::AsRawFd,
};

#[cfg(target_os = "linux")]
mod termios_linux {
    pub const TCSANOW: i32 = 0;

    pub const ICANON: u32 = 0o0002;
    pub const ECHO: u32 = 0o0010;

    pub const VMIN: usize = 6;
    pub const VTIME: usize = 5;
    pub const NCCS: usize = 32;

    #[allow(non_camel_case_types)]
    pub type tcflag_t = u32;
    #[allow(non_camel_case_types)]
    pub type cc_t = u8;
    #[allow(non_camel_case_types)]
    pub type speed_t = u32;

    #[repr(C)]
    pub struct termios {
        pub c_iflag: tcflag_t,
        pub c_oflag: tcflag_t,
        pub c_cflag: tcflag_t,
        pub c_lflag: tcflag_t,
        pub c_line: cc_t,
        pub c_cc: [cc_t; NCCS],
        pub c_ispeed: speed_t,
        pub c_ospeed: speed_t,
    }

    unsafe extern "C" {
        pub fn tcgetattr(fd: i32, termios_p: *mut termios) -> i32;
        pub fn tcsetattr(fd: i32, opt: i32, termios_p: *const termios) -> i32;
    }
}

#[cfg(target_os = "macos")]
mod termios_macos {
    pub const TCSANOW: i32 = 0;

    pub const ICANON: u32 = 0x00000100;
    pub const ECHO: u32 = 0x00000008;

    pub const VMIN: usize = 16;
    pub const VTIME: usize = 17;
    pub const NCCS: usize = 20;

    #[allow(non_camel_case_types)]
    pub type tcflag_t = u32;
    #[allow(non_camel_case_types)]
    pub type cc_t = u8;
    #[allow(non_camel_case_types)]
    pub type speed_t = u64;

    #[repr(C)]
    pub struct termios {
        pub c_iflag: tcflag_t,
        pub c_oflag: tcflag_t,
        pub c_cflag: tcflag_t,
        pub c_lflag: tcflag_t,
        pub c_cc: [cc_t; NCCS],
        pub c_ispeed: speed_t,
        pub c_ospeed: speed_t,
    }

    unsafe extern "C" {
        pub fn tcgetattr(fd: i32, termios_p: *mut termios) -> i32;
        pub fn tcsetattr(fd: i32, opt: i32, termios_p: *const termios) -> i32;
    }
}

#[cfg(target_os = "linux")]
unsafe impl Send for termios_linux::termios {}
#[cfg(target_os = "linux")]
unsafe impl Sync for termios_linux::termios {}

#[cfg(target_os = "linux")]
thread_local! {
    static ORIGINAL_TERMIOS: std::cell::RefCell<Option<termios_linux::termios>> = const { std::cell::RefCell::new(None) };
}

#[cfg(target_os = "macos")]
unsafe impl Send for termios_macos::termios {}
#[cfg(target_os = "macos")]
unsafe impl Sync for termios_macos::termios {}

#[cfg(target_os = "macos")]
thread_local! {
    static ORIGINAL_TERMIOS: std::cell::RefCell<Option<termios_macos::termios>> = const { std::cell::RefCell::new(None) };
}

fn tty_fd() -> Option<std::fs::File> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()
}

#[cfg(target_os = "linux")]
pub fn tty_raw() {
    use termios_linux::*;

    let fd = match tty_fd() {
        Some(f) => f,
        None => return,
    };

    let mut settings = std::mem::MaybeUninit::<termios>::uninit();
    unsafe {
        if tcgetattr(fd.as_raw_fd(), settings.as_mut_ptr()) != 0 {
            return;
        }
        let mut settings = settings.assume_init();

        ORIGINAL_TERMIOS.with(|original| {
            if original.borrow().is_none() {
                let mut copy = std::mem::zeroed::<termios>();
                copy.c_iflag = settings.c_iflag;
                copy.c_oflag = settings.c_oflag;
                copy.c_cflag = settings.c_cflag;
                copy.c_lflag = settings.c_lflag;
                copy.c_line = settings.c_line;
                copy.c_cc.copy_from_slice(&settings.c_cc);
                copy.c_ispeed = settings.c_ispeed;
                copy.c_ospeed = settings.c_ospeed;
                *original.borrow_mut() = Some(copy);
            }
        });

        settings.c_lflag &= !(ICANON | ECHO);
        settings.c_cc[VMIN] = 1;
        settings.c_cc[VTIME] = 0;

        tcsetattr(fd.as_raw_fd(), TCSANOW, &settings);
    }
}

#[cfg(target_os = "linux")]
pub fn tty_raw_timeout() {
    use termios_linux::*;

    let fd = match tty_fd() {
        Some(f) => f,
        None => return,
    };

    let mut settings = std::mem::MaybeUninit::<termios>::uninit();
    unsafe {
        if tcgetattr(fd.as_raw_fd(), settings.as_mut_ptr()) != 0 {
            return;
        }
        let mut settings = settings.assume_init();

        settings.c_lflag &= !(ICANON | ECHO);
        settings.c_cc[VMIN] = 0;
        settings.c_cc[VTIME] = 1;

        tcsetattr(fd.as_raw_fd(), TCSANOW, &settings);
    }
}

#[cfg(target_os = "linux")]
pub fn tty_restore() {
    use termios_linux::*;

    let fd = match tty_fd() {
        Some(f) => f,
        None => return,
    };

    ORIGINAL_TERMIOS.with(|original| {
        if let Some(ref orig) = *original.borrow() {
            unsafe {
                tcsetattr(fd.as_raw_fd(), TCSANOW, orig);
            }
        }
    });
}

#[cfg(target_os = "macos")]
pub fn tty_raw() {
    use termios_macos::*;

    let fd = match tty_fd() {
        Some(f) => f,
        None => return,
    };

    let mut settings = std::mem::MaybeUninit::<termios>::uninit();
    unsafe {
        if tcgetattr(fd.as_raw_fd(), settings.as_mut_ptr()) != 0 {
            return;
        }
        let mut settings = settings.assume_init();

        ORIGINAL_TERMIOS.with(|original| {
            if original.borrow().is_none() {
                let mut copy = std::mem::zeroed::<termios>();
                copy.c_iflag = settings.c_iflag;
                copy.c_oflag = settings.c_oflag;
                copy.c_cflag = settings.c_cflag;
                copy.c_lflag = settings.c_lflag;
                copy.c_cc.copy_from_slice(&settings.c_cc);
                copy.c_ispeed = settings.c_ispeed;
                copy.c_ospeed = settings.c_ospeed;
                *original.borrow_mut() = Some(copy);
            }
        });

        settings.c_lflag &= !(ICANON | ECHO);
        settings.c_cc[VMIN] = 1;
        settings.c_cc[VTIME] = 0;

        tcsetattr(fd.as_raw_fd(), TCSANOW, &settings);
    }
}

#[cfg(target_os = "macos")]
pub fn tty_raw_timeout() {
    use termios_macos::*;

    let fd = match tty_fd() {
        Some(f) => f,
        None => return,
    };

    let mut settings = std::mem::MaybeUninit::<termios>::uninit();
    unsafe {
        if tcgetattr(fd.as_raw_fd(), settings.as_mut_ptr()) != 0 {
            return;
        }
        let mut settings = settings.assume_init();

        settings.c_lflag &= !(ICANON | ECHO);
        settings.c_cc[VMIN] = 0;
        settings.c_cc[VTIME] = 1;

        tcsetattr(fd.as_raw_fd(), TCSANOW, &settings);
    }
}

#[cfg(target_os = "macos")]
pub fn tty_restore() {
    use termios_macos::*;

    let fd = match tty_fd() {
        Some(f) => f,
        None => return,
    };

    ORIGINAL_TERMIOS.with(|original| {
        if let Some(ref orig) = *original.borrow() {
            unsafe {
                tcsetattr(fd.as_raw_fd(), TCSANOW, orig);
            }
        }
    });
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
