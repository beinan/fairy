pub use futures_util_fork::poll_fn;

mod futures_util_fork {
    use core::{fmt, mem, pin::Pin};
    use std::{
        future::Future,
        task::{Context, Poll},
    };

    /// Future for the [`poll_fn`] function.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct PollFn<F> {
        f: F,
    }

    impl<F> Unpin for PollFn<F> {}

    /// Creates a new future wrapping around a function returning [`Poll`].
    ///
    /// Polling the returned future delegates to the wrapped function.
    pub fn poll_fn<T, F>(f: F) -> PollFn<F>
    where
        F: FnMut(&mut Context<'_>) -> Poll<T>,
    {
        PollFn { f }
    }

    impl<F> fmt::Debug for PollFn<F> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("PollFn").finish()
        }
    }

    impl<T, F> Future for PollFn<F>
    where
        F: FnMut(&mut Context<'_>) -> Poll<T>,
    {
        type Output = T;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
            (self.f)(cx)
        }
    }

    /// A future that may have completed.
    ///
    /// This is created by the [`maybe_done()`] function.
    #[allow(dead_code)]
    #[derive(Debug)]
    pub enum MaybeDone<Fut: Future> {
        /// A not-yet-completed future
        Future(/* #[pin] */ Fut),
        /// The output of the completed future
        Done(Fut::Output),
        /// The empty variant after the result of a [`MaybeDone`] has been
        /// taken using the [`take_output`](MaybeDone::take_output) method.
        Gone,
    }

    impl<Fut: Future + Unpin> Unpin for MaybeDone<Fut> {}

    /// Wraps a future into a `MaybeDone`
    #[allow(dead_code)]
    pub fn maybe_done<Fut: Future>(future: Fut) -> MaybeDone<Fut> {
        MaybeDone::Future(future)
    }

    impl<Fut: Future> MaybeDone<Fut> {
        /// Returns an [`Option`] containing a mutable reference to the output
        /// of the future. The output of this method will be [`Some`] if
        /// and only if the inner future has been completed and
        /// [`take_output`](MaybeDone::take_output) has not yet been
        /// called.
        #[allow(dead_code)]
        #[inline]
        pub fn output_mut(self: Pin<&mut Self>) -> Option<&mut Fut::Output> {
            unsafe {
                match self.get_unchecked_mut() {
                    MaybeDone::Done(res) => Some(res),
                    _ => None,
                }
            }
        }

        /// Attempt to take the output of a `MaybeDone` without driving it
        /// towards completion.
        #[allow(dead_code)]
        #[inline]
        pub fn take_output(self: Pin<&mut Self>) -> Option<Fut::Output> {
            match &*self {
                Self::Done(_) => {}
                Self::Future(_) | Self::Gone => return None,
            }
            unsafe {
                match mem::replace(self.get_unchecked_mut(), Self::Gone) {
                    MaybeDone::Done(output) => Some(output),
                    _ => unreachable!(),
                }
            }
        }
    }

    impl<Fut: Future> Future for MaybeDone<Fut> {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            unsafe {
                match self.as_mut().get_unchecked_mut() {
                    MaybeDone::Future(f) => {
                        let res = ready!(Pin::new_unchecked(f).poll(cx));
                        self.set(Self::Done(res));
                    }
                    MaybeDone::Done(_) => {}
                    MaybeDone::Gone => panic!("MaybeDone polled after value taken"),
                }
            }
            Poll::Ready(())
        }
    }
}
