#[cfg(not(feature = "std"))]
use core::{
    fmt::{self},
    slice,
    str,
};

extern "C" {
    /// [Linux man page](https://linux.die.net/man/3/lockf)
    fn lockf(
        fd: libc::c_int,
        cmd: libc::c_int,
        offset: libc::off_t,
    ) -> libc::c_int;
}

/// A type representing file descriptor on Unix.
pub type FileDesc = libc::c_int;

#[cfg(feature = "std")]
/// An IO error.
pub type Error = std::io::Error;

#[cfg(not(feature = "std"))]
#[derive(Debug)]
/// An IO error. Without std, you can only get a message or an OS error code.
pub struct Error {
    code: i32,
}

#[cfg(not(feature = "std"))]
impl Error {
    /// Creates an error from a raw OS error code.
    pub fn from_raw_os_error(code: i32) -> Self {
        Self { code }
    }

    /// Creates an error from the last OS error code.
    pub fn last_os_error() -> Error {
        Self::from_raw_os_error(unsafe { *libc::__errno_location() as i32 })
    }

    /// Raw OS error code. Returns option for compatibility with std.
    pub fn raw_os_error(&self) -> Option<i32> {
        Some(self.code)
    }
}

#[cfg(not(feature = "std"))]
impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let msg_ptr = unsafe { libc::strerror(self.code as libc::c_int) };
        let len = unsafe { libc::strlen(msg_ptr) };
        let slice = unsafe { slice::from_raw_parts(msg_ptr as _, len) };

        let mut sub = slice;

        while sub.len() > 0 {
            match str::from_utf8(sub) {
                Ok(string) => {
                    write!(fmt, "{}", string)?;
                    sub = &[];
                },
                Err(err) => {
                    let string = str::from_utf8(&sub[.. err.valid_up_to()])
                        .expect("Inconsistent utf8 error");
                    write!(fmt, "{}�", string,)?;

                    sub = &sub[err.valid_up_to() + 1 ..];
                },
            }
        }

        Ok(())
    }
}

/// Path must not contain a nul-byte in the middle, but a nul-byte in the end
/// (and only in the end) is allowed, which in this case no extra allocation
/// will be made. Otherwise, an extra allocation is made.
fn with_cstr_path<F, T>(path: &[u8], wrapped: F) -> Result<T, Error>
where
    F: FnOnce(*const i8) -> Result<T, Error>,
{
    if let Some((&last, init)) = path.split_last() {
        if init.contains(&0) {
            panic!("Path to file cannot contain nul-byte in the middle");
        }
        if last == 0 {
            return wrapped(path.as_ptr() as _);
        }
    }

    let alloc = unsafe { libc::malloc(path.len() + 1) };
    if alloc.is_null() {
        return Err(Error::last_os_error());
    }
    unsafe {
        libc::memcpy(alloc, path.as_ptr() as *const libc::c_void, path.len());
        *(alloc as *mut i8).add(path.len()) = 0;
    }

    let ret = wrapped(alloc as _);
    unsafe { libc::free(alloc) }
    ret
}

/// Opens a file with only purpose of locking it. Creates it if it does not
/// exist. Path must not contain a nul-byte in the middle, but a nul-byte in the
/// end (and only in the end) is allowed, which in this case no extra allocation
/// will be made. Otherwise, an extra allocation is made.
pub fn open(path: &[u8]) -> Result<FileDesc, Error> {
    with_cstr_path(path, |ptr| {
        let fd = unsafe {
            libc::open(
                ptr,
                libc::O_WRONLY | libc::O_CLOEXEC | libc::O_CREAT,
                libc::S_IRUSR | libc::S_IWUSR | libc::S_IRGRP | libc::S_IROTH,
            )
        };

        if fd >= 0 {
            Ok(fd)
        } else {
            Err(Error::last_os_error())
        }
    })
}

/// Tries to lock a file and blocks until it is possible to lock.
pub fn lock(fd: FileDesc) -> Result<(), Error> {
    let res = unsafe { lockf(fd, libc::F_LOCK, 0) };
    if res == 0 {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}

/// Tries to lock a file but returns as soon as possible if already locked.
pub fn try_lock(fd: FileDesc) -> Result<bool, Error> {
    let res = unsafe { lockf(fd, libc::F_TLOCK, 0) };
    if res == 0 {
        Ok(true)
    } else {
        let err = unsafe { *libc::__errno_location() };
        if err == libc::EACCES || err == libc::EAGAIN {
            Ok(false)
        } else {
            Err(Error::from_raw_os_error(err as i32))
        }
    }
}

/// Unlocks the file.
pub fn unlock(fd: FileDesc) -> Result<(), Error> {
    let res = unsafe { lockf(fd, libc::F_ULOCK, 0) };
    if res == 0 {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}

/// Removes a file. Path must not contain a nul-byte in the middle, but a
/// nul-byte in the end (and only in the end) is allowed, which in this case no
/// extra allocation will be made. Otherwise, an extra allocation is made.
pub fn remove(path: &[u8]) -> Result<(), Error> {
    with_cstr_path(path, |ptr| {
        let res = unsafe { libc::remove(ptr) };
        if res == 0 {
            Ok(())
        } else {
            Err(Error::last_os_error())
        }
    })
}

/// Closes the file.
pub fn close(fd: FileDesc) {
    unsafe { libc::close(fd) };
}
