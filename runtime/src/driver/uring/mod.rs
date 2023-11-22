//! borrowed from monoio, tokio-rs/io-uring and glommio

mod lifecycle;

use std::{
    cell::UnsafeCell,
    io,
    mem::ManuallyDrop,
    os::unix::prelude::{AsRawFd, RawFd},
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};

use io_uring::{cqueue, opcode, types::Timespec, IoUring};
use lifecycle::Lifecycle;
use log::trace;

use super::{
    op::{CompletionMeta, Op, OpAble},
    util::timespec,
    Driver, Inner, CURRENT,
};
use crate::utils::slab::Slab;

#[allow(unused)]
pub(crate) const CANCEL_USERDATA: u64 = u64::MAX;
#[allow(unused)]
pub(crate) const TIMEOUT_USERDATA: u64 = u64::MAX - 1;
#[allow(unused)]
pub(crate) const EVENTFD_USERDATA: u64 = u64::MAX - 2;

pub(crate) const MIN_REVERSED_USERDATA: u64 = u64::MAX - 2;

pub struct IoUringDriver {
    inner: Rc<UnsafeCell<UringInner>>,

    // Used as timeout buffer
    timespec: *mut Timespec,
}

pub(crate) struct UringInner {
    /// In-flight operations
    ops: Ops,

    /// IoUring bindings
    uring: ManuallyDrop<IoUring>,
}

// When dropping the driver, all in-flight operations must have completed. This
// type wraps the slab and ensures that, on drop, the slab is empty.
struct Ops {
    slab: Slab<Lifecycle>,
}

impl IoUringDriver {
    const DEFAULT_ENTRIES: u32 = 1024;

    pub(crate) fn new(b: &io_uring::Builder) -> io::Result<IoUringDriver> {
        Self::new_with_entries(b, Self::DEFAULT_ENTRIES)
    }

    #[cfg(not(feature = "sync"))]
    pub(crate) fn new_with_entries(
        urb: &io_uring::Builder,
        entries: u32,
    ) -> io::Result<IoUringDriver> {
        let uring = ManuallyDrop::new(urb.build(entries)?);

        let inner = Rc::new(UnsafeCell::new(UringInner {
            ops: Ops::new(),
            uring,
        }));

        Ok(IoUringDriver {
            inner,
            timespec: Box::leak(Box::new(Timespec::new())) as *mut Timespec,
        })
    }

    #[allow(unused)]
    fn num_operations(&self) -> usize {
        let inner = self.inner.get();
        unsafe { (*inner).ops.slab.len() }
    }

    // Flush to make enough space
    fn flush_space(inner: &mut UringInner, need: usize) -> io::Result<()> {
        let sq = inner.uring.submission();
        debug_assert!(sq.capacity() >= need);
        if sq.len() + need > sq.capacity() {
            drop(sq);
            inner.submit()?;
        }
        Ok(())
    }

    #[allow(unused)]
    fn install_timeout(&self, inner: &mut UringInner, duration: Duration) {
        let timespec = timespec(duration);
        unsafe {
            std::ptr::replace(self.timespec, timespec);
        }
        let entry = opcode::Timeout::new(self.timespec as *const Timespec)
            .build()
            .user_data(TIMEOUT_USERDATA);

        let mut sq = inner.uring.submission();
        let _ = unsafe { sq.push(&entry) };
    }

    fn inner_park(&self, timeout: Option<Duration>) -> io::Result<()> {
        let inner = unsafe { &mut *self.inner.get() };

        #[allow(unused_mut)]
        let mut need_wait = true;

        if need_wait {
            let mut space = 0;
            if timeout.is_some() {
                space += 1;
            }
            if space != 0 {
                Self::flush_space(inner, space)?;
            }

            if let Some(duration) = timeout {
                let timespec = timespec(duration);
                let args = io_uring::types::SubmitArgs::new().timespec(&timespec);
                if let Err(e) = inner.uring.submitter().submit_with_args(1, &args) {
                    if e.raw_os_error() != Some(libc::ETIME) {
                        return Err(e);
                    }
                }
            } else {
                // Submit and Wait without timeout
                inner.uring.submit_and_wait(1)?;
            }
        } else {
            // Submit only
            inner.uring.submit()?;
        }
        // Process CQ
        inner.tick();
        Ok(())
    }
}

impl Driver for IoUringDriver {
    /// Enter the driver context. This enables using uring types.
    fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        // TODO(ihciah): remove clone
        let inner = Inner::Uring(self.inner.clone());
        CURRENT.set(&inner, f)
    }

    fn submit(&self) -> io::Result<()> {
        let inner = unsafe { &mut *self.inner.get() };
        inner.submit()?;
        inner.tick();
        Ok(())
    }

    fn park(&self) -> io::Result<()> {
        self.inner_park(None)
    }

    fn park_timeout(&self, duration: Duration) -> io::Result<()> {
        self.inner_park(Some(duration))
    }

    #[cfg(feature = "sync")]
    type Unpark = waker::UnparkHandle;

    #[cfg(feature = "sync")]
    fn unpark(&self) -> Self::Unpark {
        UringInner::unpark(&self.inner)
    }
}

impl UringInner {
    fn tick(&mut self) {
        let cq = self.uring.completion();

        for cqe in cq {
            let index = cqe.user_data();
            match index {
                #[cfg(feature = "sync")]
                EVENTFD_USERDATA => self.eventfd_installed = false,
                _ if index >= MIN_REVERSED_USERDATA => (),
                _ => self.ops.complete(index as _, resultify(&cqe), cqe.flags()),
            }
        }
    }

    fn submit(&mut self) -> io::Result<()> {
        loop {
            match self.uring.submit() {
                Err(ref e)
                    if e.kind() == io::ErrorKind::Other
                        || e.kind() == io::ErrorKind::ResourceBusy =>
                {
                    self.tick();
                }
                e => return e.map(|_| ()),
            }
        }
    }

    fn new_op<T>(data: T, inner: &mut UringInner, driver: Inner) -> Op<T> {
        Op {
            driver,
            index: inner.ops.insert(),
            data: Some(data),
        }
    }

    pub(crate) fn submit_with_data<T>(
        this: &Rc<UnsafeCell<UringInner>>,
        data: T,
    ) -> io::Result<Op<T>>
    where
        T: OpAble,
    {
        let inner = unsafe { &mut *this.get() };
        // If the submission queue is full, flush it to the kernel
        if inner.uring.submission().is_full() {
            inner.submit()?;
        }

        // Create the operation
        let mut op = Self::new_op(data, inner, Inner::Uring(this.clone()));

        // Configure the SQE
        let data_mut = unsafe { op.data.as_mut().unwrap_unchecked() };
        let sqe = OpAble::uring_op(data_mut).user_data(op.index as _);

        {
            let mut sq = inner.uring.submission();

            // Push the new operation
            if unsafe { sq.push(&sqe).is_err() } {
                unimplemented!("when is this hit?");
            }
        }

        // Submit the new operation. At this point, the operation has been
        // pushed onto the queue and the tail pointer has been updated, so
        // the submission entry is visible to the kernel. If there is an
        // error here (probably EAGAIN), we still return the operation. A
        // future `io_uring_enter` will fully submit the event.

        // CHIHAI: We are not going to do syscall now. If we are waiting
        // for IO, we will submit on `park`.
        // let _ = inner.submit();
        Ok(op)
    }

    pub(crate) fn poll_op(
        this: &Rc<UnsafeCell<UringInner>>,
        index: usize,
        cx: &mut Context<'_>,
    ) -> Poll<CompletionMeta> {
        let inner = unsafe { &mut *this.get() };
        let lifecycle = unsafe { inner.ops.slab.get(index).unwrap_unchecked() };
        lifecycle.poll_op(cx)
    }

    pub(crate) fn drop_op<T: 'static>(
        this: &Rc<UnsafeCell<UringInner>>,
        index: usize,
        data: &mut Option<T>,
    ) {
        let inner = unsafe { &mut *this.get() };
        if index == usize::MAX {
            // already finished
            return;
        }
        if let Some(lifecycle) = inner.ops.slab.get(index) {
            let _must_finished = lifecycle.drop_op(data);
            #[cfg(feature = "async-cancel")]
            if !_must_finished {
                unsafe {
                    let cancel = opcode::AsyncCancel::new(index as u64)
                        .build()
                        .user_data(u64::MAX);

                    // Try push cancel, if failed, will submit and re-push.
                    if inner.uring.submission().push(&cancel).is_err() {
                        let _ = inner.submit();
                        let _ = inner.uring.submission().push(&cancel);
                    }
                }
            }
        }
    }

    pub(crate) unsafe fn cancel_op(this: &Rc<UnsafeCell<UringInner>>, index: usize) {
        let inner = &mut *this.get();
        let cancel = opcode::AsyncCancel::new(index as u64)
            .build()
            .user_data(u64::MAX);
        if inner.uring.submission().push(&cancel).is_err() {
            let _ = inner.submit();
            let _ = inner.uring.submission().push(&cancel);
        }
    }

    #[cfg(feature = "sync")]
    pub(crate) fn unpark(this: &Rc<UnsafeCell<UringInner>>) -> waker::UnparkHandle {
        let inner = unsafe { &*this.get() };
        let weak = std::sync::Arc::downgrade(&inner.shared_waker);
        waker::UnparkHandle(weak)
    }
}

impl AsRawFd for IoUringDriver {
    fn as_raw_fd(&self) -> RawFd {
        unsafe { (*self.inner.get()).uring.as_raw_fd() }
    }
}

impl Drop for IoUringDriver {
    fn drop(&mut self) {
        trace!("MONOIO DEBUG[IoUringDriver]: drop");

        // Dealloc leaked memory
        unsafe { std::ptr::drop_in_place(self.timespec) };

        #[cfg(feature = "sync")]
        unsafe {
            std::ptr::drop_in_place(self.eventfd_read_dst)
        };

        // Deregister thread id
        #[cfg(feature = "sync")]
        {
            use crate::driver::thread::{unregister_unpark_handle, unregister_waker_sender};
            unregister_unpark_handle(self.thread_id);
            unregister_waker_sender(self.thread_id);
        }
    }
}

impl Drop for UringInner {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.uring);
        }
    }
}

impl Ops {
    const fn new() -> Self {
        Ops { slab: Slab::new() }
    }

    // Insert a new operation
    pub(crate) fn insert(&mut self) -> usize {
        self.slab.insert(Lifecycle::Submitted)
    }

    fn complete(&mut self, index: usize, result: io::Result<u32>, flags: u32) {
        let lifecycle = unsafe { self.slab.get(index).unwrap_unchecked() };
        lifecycle.complete(result, flags);
    }
}

#[inline]
fn resultify(cqe: &cqueue::Entry) -> io::Result<u32> {
    let res = cqe.result();

    if res >= 0 {
        Ok(res as u32)
    } else {
        Err(io::Error::from_raw_os_error(-res))
    }
}
