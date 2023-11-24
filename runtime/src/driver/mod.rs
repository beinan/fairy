//! borrowed from monoio, tokio-rs/io-uring and glommio

use crate::driver::op::{CompletionMeta, Op, OpAble};
use crate::driver::uring::UringInner;
use crate::scoped_thread_local;
use std::{
    io,
    task::{Context, Poll},
    time::Duration,
};

pub(crate) mod file;
pub(crate) mod op;
pub(crate) mod shared_fd;
pub(crate) mod uring;
mod util;

/// Core driver trait.
pub trait Driver {
    /// Run with driver TLS.
    fn with<R>(&self, f: impl FnOnce() -> R) -> R;
    /// Submit ops to kernel and process returned events.
    fn submit(&self) -> io::Result<()>;
    /// Wait infinitely and process returned events.
    fn park(&self) -> io::Result<()>;
    /// Wait with timeout and process returned events.
    fn park_timeout(&self, duration: Duration) -> io::Result<()>;
}

scoped_thread_local!(pub(crate) static CURRENT: Inner);

pub(crate) enum Inner {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    Uring(std::rc::Rc<std::cell::UnsafeCell<UringInner>>),
}

impl Inner {
    fn submit_with<T: OpAble>(&self, data: T) -> io::Result<Op<T>> {
        match self {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            Inner::Uring(this) => UringInner::submit_with_data(this, data),
        }
    }

    #[allow(unused)]
    fn poll_op<T: OpAble>(
        &self,
        data: &mut T,
        index: usize,
        cx: &mut Context<'_>,
    ) -> Poll<CompletionMeta> {
        match self {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            Inner::Uring(this) => UringInner::poll_op(this, index, cx),
        }
    }

    #[allow(unused)]
    fn drop_op<T: 'static>(&self, index: usize, data: &mut Option<T>) {
        match self {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            Inner::Uring(this) => UringInner::drop_op(this, index, data),
        }
    }

    #[allow(unused)]
    pub(super) unsafe fn cancel_op(&self, op_canceller: &op::OpCanceller) {
        match self {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            Inner::Uring(this) => UringInner::cancel_op(this, op_canceller.index),
        }
    }
}
