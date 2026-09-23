//! Routes libmdbx's internal log messages to the [log] crate.

use libc::{c_char, c_int, c_uint};
use log::Level;
use std::{borrow::Cow, ffi::CStr, slice, sync::Once};

/// The `log` target of forwarded libmdbx messages.
const TARGET: &str = "libmdbx";

/// Size of the buffer libmdbx formats each message into; longer messages are
/// truncated.
const BUFFER_LEN: usize = 1024;

/// Installs the forwarding logger, once per process, before the first
/// environment is created.
///
/// libmdbx's default logger writes to stderr. This keeps libmdbx's log level
/// but hands messages to `log` under the `libmdbx` target instead, so they
/// appear only where the application installed a logger.
pub(crate) fn install() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        // Owned by libmdbx for the rest of the process. It formats into the
        // buffer only under its internal debug lock, and Rust reads it only
        // through the `msg` pointer handed to `forward`.
        let buffer: &'static mut [c_char] = Box::leak(vec![0; BUFFER_LEN].into_boxed_slice());
        unsafe {
            ffi::mdbx_setup_debug_nofmt(
                ffi::MDBX_LOG_DONTCHANGE,
                ffi::MDBX_DBG_DONTCHANGE,
                Some(forward),
                buffer.as_mut_ptr(),
                buffer.len(),
            );
        }
    });
}

unsafe extern "C" fn forward(
    level: ffi::MDBX_log_level_t,
    function: *const c_char,
    line: c_int,
    msg: *const c_char,
    length: c_uint,
) {
    let level = match level {
        ffi::MDBX_LOG_FATAL | ffi::MDBX_LOG_ERROR => Level::Error,
        ffi::MDBX_LOG_WARN => Level::Warn,
        ffi::MDBX_LOG_NOTICE => Level::Info,
        ffi::MDBX_LOG_VERBOSE | ffi::MDBX_LOG_DEBUG => Level::Debug,
        _ => Level::Trace,
    };
    if !log::log_enabled!(target: TARGET, level) {
        return;
    }
    // `length` is vsnprintf's result, which exceeds the buffer (minus its
    // NUL terminator) when the message was truncated.
    let len = (length as usize).min(BUFFER_LEN - 1);
    let msg = String::from_utf8_lossy(unsafe { slice::from_raw_parts(msg.cast::<u8>(), len) });
    let msg = msg.trim_end();
    let function: Option<Cow<str>> =
        (!function.is_null()).then(|| unsafe { CStr::from_ptr(function) }.to_string_lossy());
    match function {
        Some(function) => log::log!(target: TARGET, level, "{function}:{line}: {msg}"),
        None => log::log!(target: TARGET, level, "{msg}"),
    }
}
