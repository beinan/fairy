use std::io::IoSlice;

pub mod reply_attr;
pub mod reply_data;
pub mod reply_entry;
pub mod reply_ops;
pub mod reply_raw;

pub trait Reply {
    /// Create a new reply for the given request
    fn new<S: ReplySender>(unique: u64, sender: S) -> Self;
}

pub trait ReplySender: Send + Sync + Unpin + 'static {
    /// Send data.
    fn send(&self, data: &[IoSlice<'_>]) -> std::io::Result<()>;
}
