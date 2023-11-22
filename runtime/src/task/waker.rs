use std::cell::Cell;

use std::{
    future::Future,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops,
    ptr::NonNull,
    task::{RawWaker, RawWakerVTable, Waker},
};

use super::{core::Header, harness::Harness, Schedule};

pub(super) struct WakerRef<'a, S: 'static> {
    waker: ManuallyDrop<Waker>,
    _p: PhantomData<(&'a Header, S)>,
}

/// Returns a `WakerRef` which avoids having to pre-emptively increase the
/// refcount if there is no need to do so.
pub(super) fn waker_ref<T, S>(header: &Header) -> WakerRef<'_, S>
where
    T: Future,
    S: Schedule,
{
    // `Waker::will_wake` uses the VTABLE pointer as part of the check. This
    // means that `will_wake` will always return false when using the current
    // task's waker. (discussion at rust-lang/rust#66281).
    //
    // To fix this, we use a single vtable. Since we pass in a reference at this
    // point and not an *owned* waker, we must ensure that `drop` is never
    // called on this waker instance. This is done by wrapping it with
    // `ManuallyDrop` and then never calling drop.
    let waker = unsafe { ManuallyDrop::new(Waker::from_raw(raw_waker::<T, S>(header))) };

    WakerRef {
        waker,
        _p: PhantomData,
    }
}

impl<S> ops::Deref for WakerRef<'_, S> {
    type Target = Waker;

    fn deref(&self) -> &Waker {
        &self.waker
    }
}

unsafe fn clone_waker<T, S>(ptr: *const ()) -> RawWaker
where
    T: Future,
    S: Schedule,
{
    let header = ptr as *const Header;
    (*header).state.ref_inc();
    raw_waker::<T, S>(header)
}

unsafe fn drop_waker<T, S>(ptr: *const ())
where
    T: Future,
    S: Schedule,
{
    let ptr = NonNull::new_unchecked(ptr as *mut Header);
    let harness = Harness::<T, S>::from_raw(ptr);
    harness.drop_reference();
}

unsafe fn wake_by_val<T, S>(ptr: *const ())
where
    T: Future,
    S: Schedule,
{
    let ptr = NonNull::new_unchecked(ptr as *mut Header);
    let harness = Harness::<T, S>::from_raw(ptr);
    harness.wake_by_val();
}

// Wake without consuming the waker
unsafe fn wake_by_ref<T, S>(ptr: *const ())
where
    T: Future,
    S: Schedule,
{
    let ptr = NonNull::new_unchecked(ptr as *mut Header);
    let harness = Harness::<T, S>::from_raw(ptr);
    harness.wake_by_ref();
}

pub(super) fn raw_waker<T, S>(header: *const Header) -> RawWaker
where
    T: Future,
    S: Schedule,
{
    let ptr = header as *const ();
    let vtable = &RawWakerVTable::new(
        clone_waker::<T, S>,
        wake_by_val::<T, S>,
        wake_by_ref::<T, S>,
        drop_waker::<T, S>,
    );
    RawWaker::new(ptr, vtable)
}

/// Creates a waker that does nothing.
///
/// This `Waker` is useful for polling a `Future` to check whether it is
/// `Ready`, without doing any additional work.
pub(crate) fn dummy_waker() -> Waker {
    fn raw_waker() -> RawWaker {
        // the pointer is never dereferenced, so null is ok
        RawWaker::new(std::ptr::null::<()>(), vtable())
    }

    fn vtable() -> &'static RawWakerVTable {
        &RawWakerVTable::new(
            |_| raw_waker(),
            |_| {
                set_poll();
            },
            |_| {
                set_poll();
            },
            |_| {},
        )
    }

    unsafe { Waker::from_raw(raw_waker()) }
}

#[thread_local]
static SHOULD_POLL: Cell<bool> = Cell::new(true);

#[inline]
pub(crate) fn should_poll() -> bool {
    SHOULD_POLL.replace(false)
}

#[inline]
pub(crate) fn set_poll() {
    SHOULD_POLL.set(true);
}
