//! Heavily borrowed from tokio-uring & monoio.
//! Utilities for working with buffers.
//!
//! `io_uring` APIs require passing ownership of buffers to the runtime. The
//! crate defines [`IoBuf`] and [`IoBufMut`] traits which are implemented by
//! buffer types that respect the `io_uring` contract.
// Copyright (c) 2021 Tokio-uring Contributors, licensed under the MIT license.

mod io_buf;
pub use io_buf::{IoBuf, IoBufMut};

mod io_vec_buf;

#[allow(unused_imports)]
pub use io_vec_buf::{IoVecBuf, IoVecBufMut, VecBuf};

mod slice;

#[allow(unused_imports)]
pub use slice::{IoVecWrapper, IoVecWrapperMut, Slice, SliceMut};

mod raw_buf;

#[allow(unused_imports)]
pub use raw_buf::{RawBuf, RawBufVectored};

pub type BufResult<T, B> = (std::io::Result<T>, B);

pub(crate) fn deref(buf: &impl IoBuf) -> &[u8] {
    // Safety: the `IoBuf` trait is marked as unsafe and is expected to be
    // implemented correctly.
    unsafe { std::slice::from_raw_parts(buf.read_ptr(), buf.bytes_init()) }
}
