mod core;
mod harness;
pub(crate) mod join;
mod raw;
mod state;
mod utils;
pub(crate) mod waker;

use std::{future::Future, marker::PhantomData, ptr::NonNull};

use self::core::{Cell, Header};
use self::join::JoinHandle;
use self::raw::RawTask;

/// An owned handle to the task, tracked by ref count, not sendable
#[repr(transparent)]
pub(crate) struct Task<S: 'static> {
    raw: RawTask,
    _p: PhantomData<S>,
}

impl<S: 'static> Task<S> {
    #[allow(unused)]
    unsafe fn from_raw(ptr: NonNull<Header>) -> Task<S> {
        Task {
            raw: RawTask::from_raw(ptr),
            _p: PhantomData,
        }
    }

    fn header(&self) -> &Header {
        self.raw.header()
    }

    pub(crate) fn run(self) {
        self.raw.poll();
    }

    #[cfg(feature = "sync")]
    pub(crate) unsafe fn finish(&mut self, val_slot: *mut ()) {
        self.raw.finish(val_slot);
    }
}

impl<S: 'static> Drop for Task<S> {
    fn drop(&mut self) {
        // Decrement the ref count
        if self.header().state.ref_dec() {
            // Deallocate if this is the final ref count
            self.raw.dealloc();
        }
    }
}

pub(crate) trait Schedule: Sized + 'static {
    /// Schedule the task
    fn schedule(&self, task: Task<Self>);
    /// Schedule the task to run in the near future, yielding the thread to
    /// other tasks.
    fn yield_now(&self, task: Task<Self>) {
        self.schedule(task);
    }
}

pub(crate) fn new_task<T, S>(
    owner_id: usize,
    task: T,
    scheduler: S,
) -> (Task<S>, JoinHandle<T::Output>)
where
    S: Schedule,
    T: Future + 'static,
    T::Output: 'static,
{
    unsafe { new_task_holding(owner_id, task, scheduler) }
}

pub(crate) unsafe fn new_task_holding<T, S>(
    owner_id: usize,
    task: T,
    scheduler: S,
) -> (Task<S>, JoinHandle<T::Output>)
where
    S: Schedule,
    T: Future,
{
    let raw = RawTask::new::<T, S>(owner_id, task, scheduler);
    let task = Task {
        raw,
        _p: PhantomData,
    };
    let join = JoinHandle::new(raw);

    (task, join)
}
