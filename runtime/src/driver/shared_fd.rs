use crate::ready;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::{cell::UnsafeCell, io, rc::Rc};

use super::CURRENT;

// Tracks in-flight operations on a file descriptor. Ensures all in-flight
// operations complete before submitting the close.
#[derive(Clone, Debug)]
pub(crate) struct SharedFd {
    inner: Rc<Inner>,
}

struct Inner {
    // Open file descriptor
    #[cfg(unix)]
    fd: RawFd,
    // Waker to notify when the close operation completes.
    state: UnsafeCell<State>,
}

enum State {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    Uring(UringState),
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner").field("fd", &self.fd).finish()
    }
}

#[cfg(all(target_os = "linux", feature = "iouring"))]
enum UringState {
    /// Initial state
    Init,

    /// Waiting for all in-flight operation to complete.
    Waiting(Option<std::task::Waker>),

    /// The FD is closing
    Closing(super::op::Op<super::op::close::Close>),

    /// The FD is fully closed
    Closed,
}

#[cfg(unix)]
impl AsRawFd for SharedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.raw_fd()
    }
}
impl SharedFd {
    #[cfg(unix)]
    #[allow(unreachable_code, unused)]
    pub(crate) fn new(fd: RawFd) -> io::Result<SharedFd> {
        #[cfg(all(not(feature = "legacy"), target_os = "linux", feature = "iouring"))]
        let state = State::Uring(UringState::Init);
        #[allow(unreachable_code)]
        Ok(SharedFd {
            inner: Rc::new(Inner {
                fd,
                state: UnsafeCell::new(state),
            }),
        })
    }

    #[cfg(unix)]
    #[allow(unreachable_code, unused)]
    pub(crate) fn new_without_register(fd: RawFd) -> SharedFd {
        let state = CURRENT.with(|inner| match inner {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            super::Inner::Uring(_) => State::Uring(UringState::Init),
        });

        SharedFd {
            inner: Rc::new(Inner {
                fd,
                state: UnsafeCell::new(state),
            }),
        }
    }

    #[cfg(unix)]
    /// Returns the RawFd
    pub(crate) fn raw_fd(&self) -> RawFd {
        self.inner.fd
    }

    #[allow(dead_code)]
    #[cfg(unix)]
    /// Try unwrap Rc, then deregister if registered and return rawfd.
    /// Note: this action will consume self and return rawfd without closing it.
    pub(crate) fn try_unwrap(self) -> Result<RawFd, Self> {
        use std::mem::{ManuallyDrop, MaybeUninit};

        let fd = self.inner.fd;
        match Rc::try_unwrap(self.inner) {
            Ok(inner) => {
                // Only drop Inner's state, skip its drop impl.
                let mut inner_skip_drop = ManuallyDrop::new(inner);
                #[allow(invalid_value)]
                #[allow(clippy::uninit_assumed_init)]
                let mut state = unsafe { MaybeUninit::uninit().assume_init() };
                std::mem::swap(&mut inner_skip_drop.state, &mut state);
                Ok(fd)
            }
            Err(inner) => Err(Self { inner }),
        }
    }
    #[allow(unused)]
    pub(crate) fn registered_index(&self) -> Option<usize> {
        let state = unsafe { &*self.inner.state.get() };
        match state {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            State::Uring(_) => None,
        }
    }

    /// An FD cannot be closed until all in-flight operation have completed.
    /// This prevents bugs where in-flight reads could operate on the incorrect
    /// file descriptor.
    pub(crate) async fn close(self) {
        // Here we only submit close op for uring mode.
        // Fd will be closed when Inner drops for legacy mode.
        #[cfg(all(target_os = "linux", feature = "iouring"))]
        {
            let fd = self.inner.fd;
            let mut this = self;
            #[allow(irrefutable_let_patterns)]
            if let State::Uring(uring_state) = unsafe { &mut *this.inner.state.get() } {
                if Rc::get_mut(&mut this.inner).is_some() {
                    *uring_state = match super::op::Op::close(fd) {
                        Ok(op) => UringState::Closing(op),
                        Err(_) => {
                            let _ = unsafe { std::fs::File::from_raw_fd(fd) };
                            return;
                        }
                    };
                }
                this.inner.closed().await;
            }
        }
    }
}

#[cfg(all(target_os = "linux", feature = "iouring"))]
impl Inner {
    /// Completes when the FD has been closed.
    /// Should only be called for uring mode.
    async fn closed(&self) {
        use std::task::Poll;

        crate::macros::support::poll_fn(|cx| {
            let state = unsafe { &mut *self.state.get() };

            #[allow(irrefutable_let_patterns)]
            if let State::Uring(uring_state) = state {
                use std::{future::Future, pin::Pin};
                return match uring_state {
                    UringState::Init => {
                        *uring_state = UringState::Waiting(Some(cx.waker().clone()));
                        Poll::Pending
                    }
                    UringState::Waiting(Some(waker)) => {
                        if !waker.will_wake(cx.waker()) {
                            *waker = cx.waker().clone();
                        }

                        Poll::Pending
                    }
                    UringState::Waiting(None) => {
                        *uring_state = UringState::Waiting(Some(cx.waker().clone()));
                        Poll::Pending
                    }
                    UringState::Closing(op) => {
                        // Nothing to do if the close operation failed.
                        let _ = ready!(Pin::new(op).poll(cx));
                        *uring_state = UringState::Closed;
                        Poll::Ready(())
                    }
                    UringState::Closed => Poll::Ready(()),
                };
            }
            Poll::Ready(())
        })
        .await;
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let fd = self.fd;
        let state = unsafe { &mut *self.state.get() };
        #[allow(unreachable_patterns)]
        match state {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            State::Uring(UringState::Init) | State::Uring(UringState::Waiting(..)) => {
                if super::op::Op::close(fd).is_err() {
                    let _ = unsafe { std::fs::File::from_raw_fd(fd) };
                };
            }
            _ => {}
        }
    }
}
