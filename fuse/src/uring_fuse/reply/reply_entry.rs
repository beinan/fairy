use std::time::Duration;

use libc::c_int;

use crate::uring_fuse::file_meta::FileAttr;
use crate::uring_fuse::low_level::response::Response;

use super::reply_raw::ReplyRaw;
use super::Reply;
use super::ReplySender;

pub struct ReplyEntry {
    reply: ReplyRaw,
}

impl Reply for ReplyEntry {
    fn new<S: ReplySender>(unique: u64, sender: S) -> ReplyEntry {
        ReplyEntry {
            reply: Reply::new(unique, sender),
        }
    }
}

impl ReplyEntry {
    /// Reply to a request with the given entry
    pub fn entry(self, ttl: &Duration, attr: &FileAttr, generation: u64) {
        self.reply.send_ll(&Response::new_entry(
            attr.ino,
            generation,
            &attr.into(),
            *ttl,
            *ttl,
        ));
    }

    /// Reply to a request with the given error code
    pub fn error(self, err: c_int) {
        self.reply.error(err);
    }
}
