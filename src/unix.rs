use core::{fmt, mem::transmute, ops::Deref, ptr::NonNull, slice, str};

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
        let slice = unsafe { slice::from_raw_parts(msg_ptr, len) };
        write!(fmt, "{}", unsafe { OsStr::from_slice(slice) })?;
        Ok(())
    }
}

/// Owned allocation of an OS-native string.
pub struct OsString {
    alloc: NonNull<i8>,
    /// Length without the nul-byte.
    len: usize,
}

impl Drop for OsString {
    fn drop(&mut self) {
        let ptr = self.alloc.as_ptr() as *mut libc::c_void;
        unsafe { libc::free(ptr) }
    }
}

impl fmt::Debug for OsString {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{:?}", self.as_ref())
    }
}

impl fmt::Display for OsString {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.as_ref())
    }
}

impl AsRef<OsStr> for OsString {
    fn as_ref(&self) -> &OsStr {
        unsafe {
            OsStr::from_slice(slice::from_raw_parts(
                self.alloc.as_ptr(),
                self.len,
            ))
        }
    }
}

impl Deref for OsString {
    type Target = OsStr;

    fn deref(&self) -> &OsStr {
        self.as_ref()
    }
}

/// Borrowed allocation of an OS-native string.
#[repr(transparent)]
pub struct OsStr {
    bytes: [i8],
}

impl OsStr {
    /// Unsafe cause sequence needs to end with 0.
    unsafe fn from_slice(slice: &[i8]) -> &Self {
        transmute(slice)
    }
}

impl fmt::Debug for OsStr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut first = false;
        write!(fmt, "[")?;

        for &signed in &self.bytes {
            let byte = signed as u8;
            if first {
                first = false;
            } else {
                write!(fmt, ", ")?;
            }
            if (byte).is_ascii() {
                write!(fmt, "{:?}", char::from(byte))?;
            } else {
                write!(fmt, "'\\x{:x}'", byte)?;
            }
        }

        write!(fmt, "]")?;
        Ok(())
    }
}

impl fmt::Display for OsStr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let ptr = self.bytes.as_ptr();
        let len = unsafe { libc::strlen(ptr) };
        let slice = unsafe { slice::from_raw_parts(ptr as _, len) };

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

/// Either borrowed or owned allocation of an OS-native string.
#[derive(Debug)]
pub enum EitherOsStr<'str> {
    /// Borrowed allocation.
    Borrowed(&'str OsStr),
    /// Owned allocation.
    Owned(OsString),
}

impl<'str> AsRef<OsStr> for EitherOsStr<'str> {
    fn as_ref(&self) -> &OsStr {
        match self {
            Self::Borrowed(str) => str,
            Self::Owned(string) => string.as_ref(),
        }
    }
}

impl<'str> Deref for EitherOsStr<'str> {
    type Target = OsStr;

    fn deref(&self) -> &OsStr {
        self.as_ref()
    }
}

/// Conversion of anything into an owned OS-native string. If allocation fails,
/// an error shall be returned.
pub trait IntoOsString {
    /// Converts with possible allocation error.
    fn into_os_string(self) -> Result<OsString, Error>;
}

impl IntoOsString for OsString {
    fn into_os_string(self) -> Result<OsString, Error> {
        Ok(self)
    }
}

impl<'str> IntoOsString for &'str OsStr {
    fn into_os_string(self) -> Result<OsString, Error> {
        let len = unsafe { libc::strlen(self.bytes.as_ptr()) };
        let alloc = unsafe { libc::malloc(len + 1) };
        let alloc = match NonNull::new(alloc as *mut i8) {
            Some(alloc) => alloc,
            None => {
                return Err(Error::last_os_error());
            },
        };
        unsafe {
            libc::memcpy(
                alloc.as_ptr() as *mut libc::c_void,
                self.bytes.as_ptr() as *const libc::c_void,
                len + 1,
            );
        }

        Ok(OsString { alloc, len })
    }
}

impl<'str> IntoOsString for EitherOsStr<'str> {
    fn into_os_string(self) -> Result<OsString, Error> {
        match self {
            Self::Borrowed(str) => str.into_os_string(),
            Self::Owned(string) => Ok(string),
        }
    }
}

impl<'str> IntoOsString for &'str str {
    fn into_os_string(self) -> Result<OsString, Error> {
        self.to_os_str()?.into_os_string()
    }
}

/// Conversion of anything to an either borrowed or owned OS-native string. If
/// allocation fails, an error shall be returned.
pub trait ToOsStr {
    /// Converts with possible allocation error.
    fn to_os_str(&self) -> Result<EitherOsStr, Error>;
}

impl ToOsStr for OsStr {
    fn to_os_str(&self) -> Result<EitherOsStr, Error> {
        Ok(EitherOsStr::Borrowed(self))
    }
}

impl ToOsStr for OsString {
    fn to_os_str(&self) -> Result<EitherOsStr, Error> {
        Ok(EitherOsStr::Borrowed(self.as_ref()))
    }
}

impl ToOsStr for str {
    fn to_os_str(&self) -> Result<EitherOsStr, Error> {
        make_os_str(self.as_bytes())
    }
}

/// Path must not contain a nul-byte in the middle, but a nul-byte in the end
/// (and only in the end) is allowed, which in this case no extra allocation
/// will be made. Otherwise, an extra allocation is made.
fn make_os_str(slice: &[u8]) -> Result<EitherOsStr, Error> {
    if let Some((&last, init)) = slice.split_last() {
        if init.contains(&0) {
            panic!("Path to file cannot contain nul-byte in the middle");
        }
        if last == 0 {
            let str = unsafe { OsStr::from_slice(transmute(slice)) };
            return Ok(EitherOsStr::Borrowed(str));
        }
    }

    let alloc = unsafe { libc::malloc(slice.len() + 1) };
    let alloc = match NonNull::new(alloc as *mut i8) {
        Some(alloc) => alloc,
        None => {
            return Err(Error::last_os_error());
        },
    };
    unsafe {
        libc::memcpy(
            alloc.as_ptr() as *mut libc::c_void,
            slice.as_ptr() as *const libc::c_void,
            slice.len(),
        );
        *alloc.as_ptr().add(slice.len()) = 0;
    }

    Ok(EitherOsStr::Owned(OsString { alloc, len: slice.len() }))
}

/// Opens a file with only purpose of locking it. Creates it if it does not
/// exist. Path must not contain a nul-byte in the middle, but a nul-byte in the
/// end (and only in the end) is allowed, which in this case no extra allocation
/// will be made. Otherwise, an extra allocation is made.
pub fn open(path: &OsStr) -> Result<FileDesc, Error> {
    let fd = unsafe {
        libc::open(
            path.bytes.as_ptr(),
            libc::O_WRONLY | libc::O_CLOEXEC | libc::O_CREAT,
            libc::S_IRUSR | libc::S_IWUSR | libc::S_IRGRP | libc::S_IROTH,
        )
    };

    if fd >= 0 {
        Ok(fd)
    } else {
        Err(Error::last_os_error())
    }
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
pub fn remove(path: &OsStr) -> Result<(), Error> {
    let res = unsafe { libc::remove(path.bytes.as_ptr()) };
    if res == 0 {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}

/// Closes the file.
pub fn close(fd: FileDesc) {
    unsafe { libc::close(fd) };
}
