//! The Android app's native library, `libmain.so`: SDL's Java activity
//! (`org.libsdl.app.SDLActivity`) loads `libSDL3.so`, built from source
//! beside it, then this library, and calls [`SDL_main`], which runs the
//! launcher as the desktop's executable does, without a command line.
//! Failures go to Android's log.

#![cfg(target_os = "android")]

use std::ffi::{CString, c_char, c_int};

/// Android's log priority for errors.
const LOG_ERROR: c_int = 6;
const LOG_TAG: &str = "re-zoids-saga";

#[allow(unsafe_code)]
#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

/// SDL's entry point, which its activity calls on its own thread once the
/// window can be created.
#[allow(non_snake_case, unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn SDL_main(_argc: c_int, _argv: *mut *mut c_char) -> c_int {
    match launcher::run(std::iter::empty()) {
        Ok(()) => 0,
        Err(error) => {
            log_error(&format!("{error:#}"));
            1
        }
    }
}

/// Writes `text` to Android's log as an error.
#[allow(unsafe_code)]
fn log_error(text: &str) {
    let (Ok(tag), Ok(text)) = (CString::new(LOG_TAG), CString::new(text.replace('\0', " "))) else {
        return;
    };
    // SAFETY: both pointers are to NUL-terminated strings that outlive the call.
    unsafe {
        __android_log_write(LOG_ERROR, tag.as_ptr(), text.as_ptr());
    }
}
