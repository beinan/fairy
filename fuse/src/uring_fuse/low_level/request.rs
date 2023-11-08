use std::{error, fmt, mem};

use super::{kernel_interface::fuse_in_header, errno::Errno, response::Response};

/// Error that may occur while reading and parsing a request from the kernel driver.
#[derive(Debug)]
pub enum RequestError {
    /// Not enough data for parsing header (short read).
    ShortReadHeader(usize),
    /// Kernel requested an unknown operation.
    UnknownOperation(u32),
    /// Not enough data for arguments (short read).
    ShortRead(usize, usize),
    /// Insufficient argument data.
    InsufficientData,
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestError::ShortReadHeader(len) => write!(
                f,
                "Short read of FUSE request header ({len} < {})",
                mem::size_of::<fuse_in_header>()
            ),
            RequestError::UnknownOperation(opcode) => write!(f, "Unknown FUSE opcode ({opcode})"),
            RequestError::ShortRead(len, total) => {
                write!(f, "Short read of FUSE request ({len} < {total})")
            }
            RequestError::InsufficientData => write!(f, "Insufficient argument data"),
        }
    }
}

impl error::Error for RequestError {}


pub trait Request: Sized {
    /// Returns the unique identifier of this request.
    ///
    /// The FUSE kernel driver assigns a unique id to every concurrent request. This allows to
    /// distinguish between multiple concurrent requests. The unique id of a request may be
    /// reused in later requests after it has completed.
    fn unique(&self) -> u64;

    /// Returns the node id of the inode this request is targeted to.
    fn nodeid(&self) -> u64;

    /// Returns the UID that the process that triggered this request runs under.
    fn uid(&self) -> u32;

    /// Returns the GID that the process that triggered this request runs under.
    fn gid(&self) -> u32;

    /// Returns the PID of the process that triggered this request.
    fn pid(&self) -> u32;

    /// Create an error response for this Request
    fn reply_err(&self, errno: Errno) -> Response<'_> {
        Response::new_error(errno)
    }
}