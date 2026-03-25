use super::libc::{c_int, c_void, exit, signal, size_t, write, SIGINT, STDOUT_FILENO};
use super::ALTERNATE_MODE;
use std::sync::atomic::Ordering;

extern "C" fn sigint_handler(_signum: c_int) {
    unsafe {
        if ALTERNATE_MODE.load(Ordering::SeqCst) {
            let seq = b"\x1b[?1049l";
            let _ = write(
                STDOUT_FILENO,
                seq.as_ptr() as *const c_void,
                seq.len() as size_t,
            );
        }
        let nl = b"\n";
        let _ = write(
            STDOUT_FILENO,
            nl.as_ptr() as *const c_void,
            nl.len() as size_t,
        );
        exit(1);
    }
}

pub fn init_signal_handler() {
    unsafe {
        let _ = signal(SIGINT, sigint_handler as *const () as usize);
    }
}
