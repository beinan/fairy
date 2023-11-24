use crate::driver;
use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use crate::ready;

pub(crate) mod close;
pub(crate) mod open;

/// In-flight operation
pub(crate) struct Op<T: 'static> {
    // Driver running the operation
    pub(super) driver: driver::Inner,

    // Operation index in the slab(useless for legacy)
    pub(super) index: usize,

    // Per-operation data
    pub(super) data: Option<T>,
}

/// Operation completion. Returns stored state with the result of the operation.
#[derive(Debug)]
pub(crate) struct Completion<T> {
    #[allow(unused)]
    pub(crate) data: T,
    #[allow(unused)]
    pub(crate) meta: CompletionMeta,
}

/// Operation completion meta info.
#[derive(Debug)]
pub(crate) struct CompletionMeta {
    #[allow(unused)]
    pub(crate) result: io::Result<u32>,
    #[allow(unused)]
    pub(crate) flags: u32,
}

pub(crate) trait OpAble {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry;
}

impl<T> Op<T> {
    /// Submit an operation to uring.
    ///
    /// `state` is stored during the operation tracking any state submitted to
    /// the kernel.
    pub(super) fn submit_with(data: T) -> io::Result<Op<T>>
    where
        T: OpAble,
    {
        driver::CURRENT.with(|this| this.submit_with(data))
    }

    /// Try submitting an operation to uring
    #[allow(unused)]
    pub(super) fn try_submit_with(data: T) -> io::Result<Op<T>>
    where
        T: OpAble,
    {
        if driver::CURRENT.is_set() {
            Op::submit_with(data)
        } else {
            Err(io::ErrorKind::Other.into())
        }
    }

    #[allow(unused)]
    pub(crate) fn op_canceller(&self) -> OpCanceller
    where
        T: OpAble,
    {
        #[cfg(feature = "legacy")]
        if is_legacy() {
            return if let Some((dir, id)) = self.data.as_ref().unwrap().legacy_interest() {
                OpCanceller {
                    index: id,
                    direction: Some(dir),
                }
            } else {
                OpCanceller {
                    index: 0,
                    direction: None,
                }
            };
        }
        OpCanceller {
            index: self.index,
            #[cfg(feature = "legacy")]
            direction: None,
        }
    }
}

impl<T> Future for Op<T>
where
    T: Unpin + OpAble + 'static,
{
    type Output = Completion<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;
        let data_mut = me.data.as_mut().expect("unexpected operation state");
        let meta = ready!(me.driver.poll_op::<T>(data_mut, me.index, cx));

        me.index = usize::MAX;
        let data = me.data.take().expect("unexpected operation state");
        Poll::Ready(Completion { data, meta })
    }
}

impl<T> Drop for Op<T> {
    fn drop(&mut self) {
        self.driver.drop_op(self.index, &mut self.data);
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub(crate) struct OpCanceller {
    pub(super) index: usize,
}

impl OpCanceller {
    #[allow(unused)]
    pub(crate) unsafe fn cancel(&self) {
        super::CURRENT.with(|inner| inner.cancel_op(self))
    }
}
