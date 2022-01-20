# fslock

API to use files as a lock. Supports non-std crates by disabling feature
`std`.

**This is an MFEK fork of an upstream project.**

©2019–2022, @brunoczim<br/>
©2022–, Fredrick R. Brennan &lt;copypaste@kittens.ph&gt; and MFEK Authors

# Types
Currently, only one type is provided: [`LockFile`]. It does not destroy the
file after closed and behaviour on locking different file handles owned by
the same process is different between Unix and Windows, unless you activate the
`multilock` feature, which enables the `open_excl` method that locks files per
file descriptor/handle on all platforms.

# MFEK fork additions

* `LockFile::raw`
* `(&mut LockFile) as Into::<File>`

# Example
```rust
use fslock::LockFile;
fn main() -> Result<(), fslock::Error> {

    let mut file = LockFile::open("mylock")?;
    file.lock()?;
    do_stuff();
    file.unlock()?;

    Ok(())
}
```

# Docs on Master

http://mfek.org/fslock.rlib/fslock/

# License

```
MIT License

Copyright (c) 2022 Fredrick R. Brennan and MFEK Authors
Copyright (c) 2019 brunoczim

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
