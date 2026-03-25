/* -----------------------------------------------------------------------------
 * Signal handling for Ctrl+C.
 *
 * Installs a SIGINT handler that restores the terminal (exits alternate screen
 * mode if active), prints a newline, and terminates the process.
 *
 * On Linux, uses raw syscalls via inline asm — no C runtime dependency.
 * On macOS, calls into libSystem (write, _exit, sigaction) which is linked
 * automatically.  The sigaction struct matches the BSD definition:
 *   { sa_handler: usize, sa_mask: u32, sa_flags: i32 } — 16 bytes.
 *
 * Linux note:  on x86_64, the old signal() syscall doesn't exist.  Only
 * rt_sigaction is available, and SA_RESTORER is mandatory — without it the
 * kernel rejects the call.  We provide a naked restorer thunk that invokes
 * rt_sigreturn on handler return.
 * --------------------------------------------------------------------------- */

use super::ALTERNATE_MODE;
use std::sync::atomic::Ordering;

const SIGINT: i32 = 2;
const STDOUT: i32 = 1;

// ── Linux: raw syscalls ─────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod imp {
    use core::arch::asm;

    // Linux glibc signal() uses SA_NODEFER | SA_RESETHAND.
    // On x86_64, SA_RESTORER (0x04000000) is mandatory.
    const SA_NODEFER: i32 = 0x40000000;
    const SA_RESETHAND: i32 = 0x80000000u32 as i32;
    #[cfg(target_arch = "x86_64")]
    const SA_RESTORER: i32 = 0x04000000;

    pub unsafe fn write(fd: i32, buf: *const u8, len: usize) {
        // SYS_write = 1
        unsafe {
            asm!(
                "syscall",
                in("rax") 1_usize, in("rdi") fd, in("rsi") buf, in("rdx") len,
                out("rcx") _, out("r11") _, options(nostack),
            );
        }
    }

    pub unsafe fn exit(code: i32) -> ! {
        // SYS_exit = 60  (_exit — no buffer flush, same as raw kernel exit)
        unsafe {
            asm!(
                "syscall",
                in("rax") 60_usize, in("rdi") code, options(noreturn),
            );
        }
    }

    // Kernel calls this to restore registers after signal handler returns.
    // rt_sigreturn = 15
    #[cfg(target_arch = "x86_64")]
    #[unsafe(naked)]
    unsafe extern "C" fn restorer() {
        core::arch::naked_asm!("mov rax, 15", "syscall");
    }

    // Kernel's k_sigaction (NOT glibc's — different field order):
    //   x86_64: { handler, flags, restorer, mask[128] }  = 152 bytes
    //   aarch64: { handler, flags, mask[128] }            = 144 bytes
    #[repr(C)]
    struct Sigaction {
        handler: usize,
        flags: i32,
        #[cfg(target_arch = "x86_64")]
        restorer: usize,
        mask: [u8; 128],
    }

    pub unsafe fn install(signum: i32, handler: usize) {
        // SYS_rt_sigaction = 13
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
                in("rdx") 0_usize,  // oldact = NULL
                in("r10") core::mem::size_of_val(&sa),
                out("rcx") _, out("r11") _, options(nostack),
            );
        }
    }
}

// ── macOS: libSystem ────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod imp {
    // macOS signal() uses SA_RESTART | SA_RESETHAND.
    const SA_RESTART: i32 = 0x0002;
    const SA_RESETHAND: i32 = 0x0004;

    // Matches the BSD struct sigaction { sa_handler, sa_mask, sa_flags }.
    // libSystem handles the kernel trampoline internally.
    #[repr(C)]
    struct Sigaction {
        handler: usize, // sa_sigaction / sa_handler
        mask: u32,      // sigset_t
        flags: i32,     // sa_flags
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
}

// ── Shared ──────────────────────────────────────────────────────────

extern "C" fn handler(_: i32) {
    unsafe {
        if ALTERNATE_MODE.load(Ordering::SeqCst) {
            imp::write(STDOUT, b"\x1b[?1049l".as_ptr(), 9);
        }
        imp::write(STDOUT, b"\n".as_ptr(), 1);
        imp::exit(1);
    }
}

pub fn init_signal_handler() {
    unsafe {
        imp::install(SIGINT, handler as *const () as usize);
    }
}
