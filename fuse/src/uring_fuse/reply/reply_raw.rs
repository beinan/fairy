use libc::c_int;
use log::{error, warn};

use crate::uring_fuse::low_level::{errno::Errno, response::Response};

use super::{Reply, ReplySender};

pub(crate) struct ReplyRaw {
    /// Unique id of the request to reply to
    unique: u64,
    /// Closure to call for sending the reply
    sender: Option<Box<dyn ReplySender>>,
}

impl Reply for ReplyRaw {
    fn new<S: ReplySender>(unique: u64, sender: S) -> ReplyRaw {
        let sender = Box::new(sender);
        ReplyRaw {
            unique,
            sender: Some(sender),
        }
    }
}

impl ReplyRaw {
    /// Reply to a request with the given error code and data. Must be called
    /// only once (the `ok` and `error` methods ensure this by consuming `self`)
    pub(super) fn send_ll_mut(&mut self, response: &Response<'_>) {
        assert!(self.sender.is_some());
        let sender = self.sender.take().unwrap();
        let res = response.with_iovec(self.unique, |iov| sender.send(iov));
        if let Err(err) = res {
            error!("Failed to send FUSE reply: {}", err);
        }
    }
    pub(super) fn send_ll(mut self, response: &Response<'_>) {
        self.send_ll_mut(response)
    }

    /// Reply to a request with the given error code
    pub fn error(self, err: c_int) {
        assert_ne!(err, 0);
        self.send_ll(&Response::new_error(Errno::from_i32(err)));
    }
}

impl Drop for ReplyRaw {
    fn drop(&mut self) {
        if self.sender.is_some() {
            warn!(
                "Reply not sent for operation {}, replying with I/O error",
                self.unique
            );
            self.send_ll_mut(&Response::new_error(Errno::EIO));
        }
    }
}
