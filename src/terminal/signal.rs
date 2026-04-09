/* -----------------------------------------------------------------------------
 * Signal handling for Ctrl+C.
 *
 * Installs a SIGINT handler that restores the terminal (exits alternate screen
 * mode if active), prints a newline, and terminates the process.
 *
 * On Linux, uses raw syscalls via inline asm — no C runtime dependency.
 * On macOS, calls into libSystem (write, _exit, sigaction) which is linked
 * automatically.
 * --------------------------------------------------------------------------- */

#[cfg(target_os = "linux")]
mod imp {
    use core::arch::asm;

    const SIGINT: i32 = 2;
    const STDOUT: i32 = 1;
    const SA_NODEFER: i32 = 0x40000000;
    const SA_RESETHAND: i32 = 0x80000000u32 as i32;
    #[cfg(target_arch = "x86_64")]
    const SA_RESTORER: i32 = 0x04000000;

    pub unsafe fn write(fd: i32, buf: *const u8, len: usize) {
        unsafe {
            asm!(
                "syscall",
                in("rax") 1_usize, in("rdi") fd, in("rsi") buf, in("rdx") len,
                out("rcx") _, out("r11") _, options(nostack),
            );
        }
    }

    pub unsafe fn exit(code: i32) -> ! {
        unsafe {
            asm!(
                "syscall",
                in("rax") 60_usize, in("rdi") code, options(noreturn),
            );
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[unsafe(naked)]
    unsafe extern "C" fn restorer() {
        core::arch::naked_asm!("mov rax, 15", "syscall");
    }

    #[repr(C)]
    struct Sigaction {
        handler: usize,
        flags: i32,
        #[cfg(target_arch = "x86_64")]
        restorer: usize,
        mask: [u8; 128],
    }

    pub unsafe fn install(signum: i32, handler: usize) {
        let sa = Sigaction {
            handler,
            flags: SA_NODEFER
                | SA_RESETHAND
                | if cfg!(target_arch = "x86_64") {
                    SA_RESTORER
                } else {
                    0
                },
            #[cfg(target_arch = "x86_64")]
            restorer: restorer as *const () as usize,
            mask: [0; 128],
        };
        unsafe {
            asm!(
                "syscall",
                in("rax") 13_usize,
                in("rdi") signum as usize,
                in("rsi") &sa as *const _ as usize,
                in("rdx") 0_usize,
                in("r10") core::mem::size_of_val(&sa),
                out("rcx") _, out("r11") _, options(nostack),
            );
        }
    }

    pub fn handler() {
        use crate::terminal::ALTERNATE_MODE;
        use std::sync::atomic::Ordering;
        unsafe {
            if ALTERNATE_MODE.load(Ordering::SeqCst) {
                write(STDOUT, b"\x1b[?1049l".as_ptr(), 9);
            }
            write(STDOUT, b"\n".as_ptr(), 1);
            exit(1);
        }
    }

    pub fn init_handler() {
        unsafe {
            install(SIGINT, handler as *const () as usize);
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    const SIGINT: i32 = 2;
    const STDOUT: i32 = 1;
    const SA_RESTART: i32 = 0x0002;
    const SA_RESETHAND: i32 = 0x0004;

    #[repr(C)]
    struct Sigaction {
        handler: usize,
        mask: u32,
        flags: i32,
    }

    #[link(name = "System", kind = "dylib")]
    unsafe extern "C" {
        #[link_name = "write"]
        fn sys_write(fd: i32, buf: *const u8, len: usize) -> isize;
        fn _exit(code: i32) -> !;
        fn sigaction(sig: i32, act: *const Sigaction, oact: *mut Sigaction) -> i32;
    }

    pub unsafe fn write(fd: i32, buf: *const u8, len: usize) {
        unsafe {
            sys_write(fd, buf, len);
        }
    }

    pub unsafe fn exit(code: i32) -> ! {
        unsafe {
            _exit(code);
        }
    }

    pub unsafe fn install(signum: i32, handler: usize) {
        let sa = Sigaction {
            handler,
            mask: 0,
            flags: SA_RESTART | SA_RESETHAND,
        };
        unsafe {
            sigaction(signum, &sa, core::ptr::null_mut());
        }
    }

    pub fn handler() {
        use crate::terminal::ALTERNATE_MODE;
        use std::sync::atomic::Ordering;
        unsafe {
            if ALTERNATE_MODE.load(Ordering::SeqCst) {
                write(STDOUT, b"\x1b[?1049l".as_ptr(), 9);
            }
            write(STDOUT, b"\n".as_ptr(), 1);
            exit(1);
        }
    }

    pub fn init_handler() {
        unsafe {
            install(SIGINT, handler as *const () as usize);
        }
    }
}

#[cfg(target_os = "linux")]
pub fn init_signal_handler() {
    imp::init_handler();
}

#[cfg(target_os = "macos")]
pub fn init_signal_handler() {
    imp::init_handler();
}

#[cfg(target_os = "windows")]
pub fn init_signal_handler() {
    // Windows: Ctrl+C handling is done via SetConsoleCtrlHandler
    // For minimal implementation, we rely on the panic hook in main.rs
    // which handles cleanup on abnormal termination
}
