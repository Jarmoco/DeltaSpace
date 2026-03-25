use core::arch::asm;

pub use core::ffi::c_void;

#[allow(non_camel_case_types)]
pub type c_int = i32;
#[allow(non_camel_case_types)]
pub type size_t = usize;

pub const SIGINT: c_int = 2;
pub const STDOUT_FILENO: c_int = 1;

const SA_NODEFER: c_int = 0x40000000;
const SA_RESETHAND: c_int = 0x80000000u32 as c_int;

#[cfg(target_arch = "x86_64")]
const SA_RESTORER: c_int = 0x04000000;

pub unsafe fn write(fd: c_int, buf: *const c_void, count: size_t) -> size_t {
    let ret: usize;
    unsafe {
        asm!(
            "syscall",
            in("rax") 1_usize,
            in("rdi") fd as usize,
            in("rsi") buf as usize,
            in("rdx") count,
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
            options(nostack),
        );
    }
    ret
}

pub unsafe fn exit(status: c_int) -> ! {
    unsafe {
        asm!(
            "syscall",
            in("rax") 60_usize,
            in("rdi") status as usize,
            options(noreturn),
        );
    }
}

#[cfg(target_arch = "x86_64")]
#[unsafe(naked)]
unsafe extern "C" fn sigreturn_restorer() {
    core::arch::naked_asm!("mov rax, 15", "syscall");
}

pub unsafe fn signal(signum: c_int, handler: usize) -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        let sa = [
            handler as u64,
            (SA_NODEFER | SA_RESETHAND | SA_RESTORER) as u64,
            sigreturn_restorer as *const () as usize as u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
        ];
        unsafe {
            asm!(
                "syscall",
                in("rax") 13_usize,
                in("rdi") signum as usize,
                in("rsi") sa.as_ptr() as usize,
                in("rdx") 0_usize,
                in("r10") core::mem::size_of_val(&sa),
                out("rcx") _,
                out("r11") _,
                options(nostack),
            );
        }
        0
    }

    #[cfg(target_arch = "aarch64")]
    {
        let sa = [
            handler as u64,
            (SA_NODEFER | SA_RESETHAND) as u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
            0u64,
        ];
        unsafe {
            asm!(
                "svc #0",
                in("x8") 134_usize,
                in("x0") signum as usize,
                in("x1") sa.as_ptr() as usize,
                in("x2") 0_usize,
                in("x3") core::mem::size_of_val(&sa),
                out("x4") _,
                out("x5") _,
                out("x6") _,
                out("x7") _,
                out("x9") _,
                out("x10") _,
                out("x11") _,
                out("x12") _,
                out("x13") _,
                out("x14") _,
                out("x15") _,
                options(nostack),
            );
        }
        0
    }
}
