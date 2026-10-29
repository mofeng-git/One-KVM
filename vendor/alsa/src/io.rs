use crate::alsa;
use super::error::*;
use core::{slice, ptr, fmt};
use core::cell::RefCell;
use ::alloc::rc::Rc;
use libc::{c_char, c_int};

/// [snd_output_t](http://www.alsa-project.org/alsa-doc/alsa-lib/group___output.html) wrapper
pub struct Output(*mut alsa::snd_output_t);

unsafe impl Send for Output {}

#[cfg(feature = "std")]
std::thread_local! {
    static ERROR_OUTPUT: RefCell<Option<Rc<RefCell<Output>>>> = RefCell::new(None);
}

impl Drop for Output {
    fn drop(&mut self) { unsafe { alsa::snd_output_close(self.0) }; }
}

impl Output {

    pub fn buffer_open() -> Result<Output> {
        let mut q = ptr::null_mut();
        acheck!(snd_output_buffer_open(&mut q)).map(|_| Output(q))
    }

    pub fn buffer_string<T, F: FnOnce(&[u8]) -> T>(&self, f: F) -> T {
        let b = unsafe {
            let mut q = ptr::null_mut();
            let s = alsa::snd_output_buffer_string(self.0, &mut q);
            if s == 0 { &[] } else { slice::from_raw_parts(q as *const u8, s as usize) }
        };
        f(b)
    }


    /// Installs a thread local error handler.
    ///
    /// Sometimes alsa-lib writes to stderr, but if you prefer, you can write it here instead.
    /// Should you wish to empty the buffer; just call local_error_handler again and drop the old instance.
    ///
    /// This is not available in `no-std` environments, because we use thread_local variables.
    #[cfg(feature = "std")]
    pub fn local_error_handler() -> Result<Rc<RefCell<Output>>> {
        let output = Output::buffer_open()?;
        let r = Rc::new(RefCell::new(output));
        ERROR_OUTPUT.with_borrow_mut(|e| *e = Some(r.clone()));
        unsafe { alsa::snd_lib_error_set_local(Some(our_error_handler)); }
        Ok(r)
    }
}

impl fmt::Debug for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Output(")?;
        fmt::Display::fmt(self, f)?;
        write!(f, ")")
        /* self.buffer_string(|b| f.write_str(try!(str::from_utf8(b).map_err(|_| fmt::Error)))) */
    }
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.buffer_string(|b| {
            let s = ::alloc::string::String::from_utf8_lossy(b);
            f.write_str(&*s)
        })
    }
}

pub fn output_handle(o: &Output) -> *mut alsa::snd_output_t { o.0 }

#[cfg(feature = "std")]
unsafe extern "C" fn our_error_handler(_file: *const c_char,
    _line: c_int,
    func: *const c_char,
    _err: c_int,
    fmt: *const c_char,
    arg: *mut alsa::__va_list_tag,
) {
    ERROR_OUTPUT.with_borrow(|e| {
        let b = e.as_ref().expect("ERROR_OUTPUT not set").borrow_mut();
        alsa::snd_output_puts(b.0, func);
        alsa::snd_output_puts(b.0, c": ".as_ptr());
        alsa::snd_output_vprintf(b.0, fmt, arg);
        alsa::snd_output_putc(b.0, '\n' as i32);
    })
}
