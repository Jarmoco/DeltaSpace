use std::os::unix::io::AsRawFd;

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

unsafe impl Send for termios {}
unsafe impl Sync for termios {}

thread_local! {
    static ORIGINAL_TERMIOS: std::cell::RefCell<Option<termios>> = const { std::cell::RefCell::new(None) };
}

pub fn tty_raw() {
    let fd = match super::tty_fd() {
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

pub fn tty_raw_timeout() {
    let fd = match super::tty_fd() {
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

pub fn tty_restore() {
    let fd = match super::tty_fd() {
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
