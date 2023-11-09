use std::time::Duration;

use libc::c_int;

use crate::uring_fuse::file_meta::FileAttr;
use crate::uring_fuse::low_level::response::Response;

use super::Reply;
use super::reply_raw::ReplyRaw;
use super::ReplySender;

pub struct ReplyAttr {
    reply: ReplyRaw,
}

impl Reply for ReplyAttr {
    fn new<S: ReplySender>(unique: u64, sender: S) -> ReplyAttr {
        ReplyAttr {
            reply: Reply::new(unique, sender),
        }
    }
}

impl ReplyAttr {
    /// Reply to a request with the given attribute
    pub fn attr(self, ttl: &Duration, attr: &FileAttr) {
        self.reply
            .send_ll(&Response::new_attr(ttl, &attr.into()));
    }

    /// Reply to a request with the given error code
    pub fn error(self, err: c_int) {
        self.reply.error(err);
    }
}