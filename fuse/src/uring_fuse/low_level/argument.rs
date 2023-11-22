//! Argument decomposition for FUSE operation requests.
//!
//! Helper to decompose a slice of binary data (incoming FUSE request) into multiple data
//! structures (request arguments).

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

/// An iterator that can be used to fetch typed arguments from a byte slice.
pub struct ArgumentIterator<'a> {
    data: &'a [u8],
}

impl<'a> ArgumentIterator<'a> {
    /// Create a new argument iterator for the given byte slice.
    pub fn new(data: &'a [u8]) -> ArgumentIterator<'a> {
        ArgumentIterator { data }
    }

    /// Returns the size of the remaining data.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Fetch a slice of all remaining bytes.
    pub fn fetch_all(&mut self) -> &'a [u8] {
        let bytes = self.data;
        self.data = &[];
        bytes
    }

    /// Fetch a typed argument. Returns `None` if there's not enough data left.
    pub fn fetch<T: zerocopy::FromBytes>(&mut self) -> Option<&'a T> {
        match zerocopy::LayoutVerified::<_, T>::new_from_prefix(self.data) {
            None => {
                if self.data.as_ptr() as usize % core::mem::align_of::<T>() != 0 {
                    // Panic on alignment errors as this is under the control
                    // of the programmer, we can still return None for size
                    // failures as this may be caused by insufficient external
                    // data.
                    panic!("Data unaligned");
                } else {
                    None
                }
            }
            Some((x, rest)) => {
                self.data = rest;
                Some(x.into_ref())
            }
        }
    }

    /// Fetch a slice of typed of arguments. Returns `None` if there's not enough data left.
    #[cfg(feature = "abi-7-16")]
    pub fn fetch_slice<T: zerocopy::FromBytes>(&mut self, count: usize) -> Option<&'a [T]> {
        match zerocopy::LayoutVerified::<_, [T]>::new_slice_from_prefix(self.data, count) {
            None => {
                if self.data.as_ptr() as usize % core::mem::align_of::<T>() != 0 {
                    // Panic on alignment errors as this is under the control
                    // of the programmer, we can still return None for size
                    // failures as this may be caused by insufficient external
                    // data.
                    panic!("Data unaligned");
                } else {
                    None
                }
            }
            Some((x, rest)) => {
                self.data = rest;
                Some(x.into_slice())
            }
        }
    }

    /// Fetch a (zero-terminated) string (can be non-utf8). Returns `None` if there's not enough
    /// data left or no zero-termination could be found.
    pub fn fetch_str(&mut self) -> Option<&'a OsStr> {
        let len = memchr::memchr(0, self.data)?;
        let (out, rest) = self.data.split_at(len);
        self.data = &rest[1..];
        Some(OsStr::from_bytes(out))
    }
}
