#![cfg_attr(not(feature = "std"), no_std)]

//! **WARNING**: v0.1.x is incompatible with v0.2.x onwards.
//!
//! API to use files as a lock. Supports non-std crates by disabling feature
//! `std`.
//!
//! # Types
//! Currently, only one type is provided: [`LockFile`]. It does not destroy the
//! file after closed. Locks are per-handle and not by per-process in any
//! platform. On Unix, however, under `fork` file descriptors might be
//! duplicated sharing the same lock, but `fork` is usually `unsafe` in Rust.
//!
//! # Example
//! ```
//! use fslock::LockFile;
//! fn main() -> Result<(), fslock::Error> {
//!
//!     let mut file = LockFile::open("testfiles/mylock.lock")?;
//!     file.lock()?;
//!     do_stuff();
//!     file.unlock()?;
//!
//!     Ok(())
//! }
//! # fn do_stuff() {
//! #    // doing stuff here.
//! # }
//! ```

#[cfg(test)]
mod test;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use crate::unix as sys;

mod constants;
pub use constants::lockfile_truncate;
mod string;
mod fmt;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use crate::windows as sys;

pub use crate::{
    string::{EitherOsStr, IntoOsString, ToOsStr},
    sys::{Error, FileDesc, OsStr, OsString},
};

#[derive(Debug)]
/// A handle to a file that is lockable. Does not delete the file. On both
/// Unix and Windows, the lock is held by an individual handle, and not by the
/// whole process. On Unix, however, under `fork` file descriptors might be
/// duplicated sharing the same lock, but `fork` is usually `unsafe` in Rust.
///
/// # Example
/// ```
/// # fn main() -> Result<(), fslock::Error> {
/// use fslock::LockFile;
///
/// let mut file = LockFile::open("testfiles/mylock.lock")?;
/// file.lock()?;
/// do_stuff();
/// file.unlock()?;
///
/// # Ok(())
/// # }
/// # fn do_stuff() {
/// #    // doing stuff here.
/// # }
/// ```
pub struct LockFile {
    pub truncate_on_close: bool,
    locked: bool,
    desc: sys::FileDesc,
}

// Private functions
impl LockFile {
    fn new() -> Self {
        #[allow(unused_unsafe)]
        // not safe to get state of default truncation setting on no_std
        Self {
            desc: sys::uninitialized_fd(),
            locked: false,
            truncate_on_close: unsafe {
                constants::default_lockfile_truncate_state()
            },
        }
    }
}

// Public functions
impl LockFile {
    /// Opens a file for locking, with OS-dependent locking behavior. On Unix,
    /// if the path is nul-terminated (ends with 0), no extra allocation will be
    /// made.
    ///
    /// # Compatibility
    ///
    /// This crate used to behave differently in regards to Unix and Windows,
    /// when locks on Unix were per-process and not per-handle. However, the
    /// current version locks per-handle on any platform. On Unix, however,
    /// under `fork` file descriptors might be duplicated sharing the same lock,
    /// but `fork` is usually `unsafe` in Rust.
    ///
    /// # Panics
    /// Panics if the path contains a nul-byte in a place other than the end.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    ///
    /// let mut file = LockFile::open("testfiles/regular.lock")?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Panicking Example
    ///
    /// ```should_panic
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    ///
    /// let mut file = LockFile::open("my\0lock")?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn open<P>(path: &P) -> Result<Self, Error>
    where
        P: ToOsStr + ?Sized,
    {
        let path = path.to_os_str()?;
        let desc = sys::open(path.as_ref())?;
        Ok(Self { desc, ..Self::new() })
    }

    /// Locks this file. Blocks while it is not possible to lock (i.e. someone
    /// else already owns a lock). After locked, if no attempt to unlock is
    /// made, it will be automatically unlocked on the file handle drop.
    ///
    /// # Panics
    /// Panics if this handle already owns the file.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    ///
    /// let mut file = LockFile::open("testfiles/target.lock")?;
    /// file.lock()?;
    /// do_stuff();
    /// file.unlock()?;
    ///
    /// # Ok(())
    /// # }
    /// # fn do_stuff() {
    /// #    // doing stuff here.
    /// # }
    /// ```
    ///
    /// # Panicking Example
    ///
    /// ```should_panic
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    ///
    /// let mut file = LockFile::open("testfiles/panicking.lock")?;
    /// file.lock()?;
    /// file.lock()?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn lock(&mut self) -> Result<(), Error> {
        debug_assert!(self.desc != sys::uninitialized_fd());
        if self.locked {
            panic!("Cannot lock if already owning a lock");
        }
        sys::lock(self.desc)?;
        self.locked = true;
        Ok(())
    }

    /// Locks this file and writes this process's PID into the file, which will
    /// be erased on unlock. Like [`LockFile::lock`], blocks while it is not
    /// possible to lock. After locked, if no attempt to unlock is made, it will
    /// be automatically unlocked on the file handle drop.
    ///
    /// # Panics
    /// Panics if this handle already owns the file.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    /// # #[cfg(feature = "std")]
    /// use std::fs::read_to_string;
    ///
    /// let mut file = LockFile::open("testfiles/withpid.lock")?;
    /// file.lock_with_pid()?;
    /// # #[cfg(feature = "std")]
    /// # {
    /// do_stuff()?;
    /// # }
    /// file.unlock()?;
    ///
    /// # #[cfg(feature = "std")]
    /// fn do_stuff() -> Result<(), fslock::Error> {
    ///     let mut content = read_to_string("testfiles/withpid.lock")?;
    ///     assert!(content.trim().len() > 0);
    ///     assert!(content.trim().chars().all(|ch| ch.is_ascii_digit()));
    ///     Ok(())
    /// }
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn lock_with_pid(&mut self) -> Result<(), Error> {
        debug_assert!(self.desc != sys::uninitialized_fd());
        if let Err(error) = self.lock() {
            return Err(error);
        }

        let result = writeln!(fmt::Writer(self.desc), "{}", sys::pid());
        if result.is_err() {
            let _ = self.unlock();
        }
        result
    }

    /// Locks this file. Does NOT block if it is not possible to lock (i.e.
    /// someone else already owns a lock). After locked, if no attempt to
    /// unlock is made, it will be automatically unlocked on the file handle
    /// drop.
    ///
    /// # Panics
    /// Panics if this handle already owns the file.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    ///
    /// let mut file = LockFile::open("testfiles/attempt.lock")?;
    /// if file.try_lock()? {
    ///     do_stuff();
    ///     file.unlock()?;
    /// }
    ///
    /// # Ok(())
    /// # }
    /// # fn do_stuff() {
    /// #    // doing stuff here.
    /// # }
    /// ```
    ///
    /// # Panicking Example
    ///
    /// ```should_panic
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    ///
    /// let mut file = LockFile::open("testfiles/attempt_panic.lock")?;
    /// file.lock()?;
    /// file.try_lock()?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_lock(&mut self) -> Result<bool, Error> {
        debug_assert!(self.desc != sys::uninitialized_fd());
        if self.locked {
            panic!("Cannot lock if already owning a lock");
        }
        let lock_result = sys::try_lock(self.desc);
        if let Ok(true) = lock_result {
            self.locked = true;
        }
        lock_result
    }

    /// Locks this file and writes this process's PID into the file, which will
    /// be erased on unlock. Does NOT block if it is not possible to lock (i.e.
    /// someone else already owns a lock). After locked, if no attempt to
    /// unlock is made, it will be automatically unlocked on the file handle
    /// drop.
    ///
    /// Warning: even if `constants::lockfile_truncate` is false, this still
    /// truncates.
    ///
    /// # Panics
    /// Panics if this handle already owns the file.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "std")]
    /// # use std::fs::read_to_string;
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    ///
    /// let mut file = LockFile::open("testfiles/pid_attempt.lock")?;
    /// if file.try_lock_with_pid()? {
    ///     # #[cfg(feature = "std")]
    ///     # {
    ///     do_stuff()?;
    ///     # }
    ///     file.unlock()?;
    /// }
    ///
    /// # Ok(())
    /// # }
    /// # #[cfg(feature = "std")]
    /// fn do_stuff() -> Result<(), fslock::Error> {
    ///     let mut content = read_to_string("testfiles/pid_attempt.lock")?;
    ///     assert!(content.trim().len() > 0);
    ///     assert!(content.trim().chars().all(|ch| ch.is_ascii_digit()));
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Panicking Example
    ///
    /// ```should_panic
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    ///
    /// let mut file = LockFile::open("testfiles/pid_attempt_panic.lock")?;
    /// file.lock_with_pid()?;
    /// file.try_lock_with_pid()?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Preserved Example
    ///
    /// ```
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    /// fslock::lockfile_truncate(false); // turn off truncation
    /// {
    ///     let mut file = LockFile::open("testfiles/pid_preserved.lock")?;
    ///     file.try_lock_with_pid()?;
    /// }
    /// # #[cfg(not(feature = "std"))]
    /// # return Ok(());
    /// # #[cfg(feature = "std")]
    /// # fn inner() -> Result<(), fslock::Error> {
    /// # use std::fs::File;
    /// # use std::io::Read as _;
    /// let mut s = String::new();
    /// {
    ///     File::open("testfiles/pid_preserved.lock").unwrap().read_to_string(&mut s)?;
    /// }
    /// assert_eq!(s.as_str().trim().parse::<u32>().map_err(|_|std::io::Error::new(std::io::ErrorKind::InvalidData, "😦"))?, std::process::id());
    /// # Ok(())
    /// # }
    /// # #[cfg(not(feature = "std"))]
    /// # fn inner() -> Result<(), fslock::Error> {Ok(())}
    /// # inner()
    /// # }
    /// ```
    pub fn try_lock_with_pid(&mut self) -> Result<bool, Error> {
        debug_assert!(self.desc != sys::uninitialized_fd());
        match self.try_lock() {
            Ok(true) => (),
            Ok(false) => return Ok(false),
            Err(error) => return Err(error),
        }

        let result = sys::truncate(self.desc)
            .and_then(|_| writeln!(fmt::Writer(self.desc), "{}", sys::pid()));
        if result.is_err() {
            let _ = self.unlock();
        }
        result.map(|_| true)
    }

    /// Returns whether this file handle owns the lock.
    ///
    /// # Example
    /// ```
    /// use fslock::LockFile;
    /// # fn main() -> Result<(), fslock::Error> {
    ///
    /// let mut file = LockFile::open("testfiles/maybeowned.lock")?;
    /// do_stuff_with_lock(&mut file);
    /// if !file.owns_lock() {
    ///     file.lock()?;
    ///     do_stuff();
    ///     file.unlock()?;
    /// }
    ///
    /// # Ok(())
    /// # }
    /// # fn do_stuff_with_lock(_lock: &mut LockFile) {
    /// #    // doing stuff here.
    /// # }
    /// # fn do_stuff() {
    /// #    // doing stuff here.
    /// # }
    /// ```
    pub fn owns_lock(&self) -> bool {
        debug_assert!(self.desc != sys::uninitialized_fd());
        self.locked
    }

    /// Unlocks this file. This file handle must own the file lock. If not
    /// called manually, it is automatically called on `drop`.
    ///
    /// # Panics
    /// Panics if this handle does not own the file.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    ///
    /// let mut file = LockFile::open("testfiles/endinglock.lock")?;
    /// file.lock()?;
    /// do_stuff();
    /// file.unlock()?;
    ///
    /// # Ok(())
    /// # }
    /// # fn do_stuff() {
    /// #    // doing stuff here.
    /// # }
    /// ```
    ///
    /// # Panicking Example
    ///
    /// ```should_panic
    /// # fn main() -> Result<(), fslock::Error> {
    /// use fslock::LockFile;
    ///
    /// let mut file = LockFile::open("testfiles/endinglock.lock")?;
    /// file.unlock()?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn unlock(&mut self) -> Result<(), Error> {
        debug_assert!(self.desc != sys::uninitialized_fd());
        if !self.locked {
            panic!("Attempted to unlock already locked lockfile");
        }
        self.locked = false;
        sys::unlock(self.desc)?;
        if self.truncate_on_close {
            sys::truncate(self.desc)?;
        }
        Ok(())
    }

    /// It is recommended that you use the implementation [`<&mut LockFile as
    /// Into<File>>::into`](struct.LockFile.html#impl-Into<File>) instead of
    /// this low-level function.
    pub unsafe fn raw(&self) -> FileDesc {
        debug_assert!(self.desc != sys::uninitialized_fd());
        self.desc
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        if self.locked {
            let _ = self.unlock();
        }
        sys::close(self.desc);
    }
}

// Safe because:
// 1. We never actually access the contents of the pointer that represents the
// Windows Handle.
//
// 2. We require a mutable reference to actually mutate the file
// system.

#[cfg(windows)]
unsafe impl Send for LockFile {}

#[cfg(windows)]
unsafe impl Sync for LockFile {}
