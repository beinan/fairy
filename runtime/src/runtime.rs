// borrowed from monoio, tokio-rs/io-uring and glommio

use std::future::Future;

use crate::task::join::JoinHandle;
use crate::{
    driver::Driver,
    scheduler::{LocalScheduler, TaskQueue},
    scoped_thread_local,
    task::{
        new_task,
        waker::{dummy_waker, set_poll, should_poll},
    },
};

scoped_thread_local!(pub(crate) static CURRENT: Context);

pub(crate) struct Context {
    /// Owned task set and local run queue
    pub(crate) tasks: TaskQueue,

    /// Thread id(not the kernel thread id but a generated unique number)
    pub(crate) thread_id: usize,
}

impl Context {
    #[cfg(not(feature = "sync"))]
    pub(crate) fn new() -> Self {
        let thread_id = crate::builder::BUILD_THREAD_ID.with(|id| *id);

        Self {
            thread_id,
            tasks: TaskQueue::default(),
        }
    }
}

/// Monoio runtime
pub struct Runtime<D> {
    pub(crate) context: Context,
    pub(crate) driver: D,
}

impl<D> Runtime<D> {
    pub(crate) fn new(context: Context, driver: D) -> Self {
        Self { context, driver }
    }

    /// Block on
    pub fn block_on<F>(&mut self, future: F) -> F::Output
    where
        F: Future,
        D: Driver,
    {
        assert!(
            !CURRENT.is_set(),
            "Can not start a runtime inside a runtime"
        );

        let waker = dummy_waker();
        let cx = &mut std::task::Context::from_waker(&waker);

        self.driver.with(|| {
            CURRENT.set(&self.context, || {
                #[cfg(not(feature = "sync"))]
                let join = future;
                let mut join = std::pin::pin!(join);
                set_poll();
                loop {
                    loop {
                        // Consume all tasks(with max round to prevent io starvation)
                        let mut max_round = self.context.tasks.len() * 2;
                        while let Some(t) = self.context.tasks.pop() {
                            t.run();
                            if max_round == 0 {
                                // maybe there's a looping task
                                break;
                            } else {
                                max_round -= 1;
                            }
                        }

                        // Check main future
                        while should_poll() {
                            // check if ready
                            if let std::task::Poll::Ready(t) = join.as_mut().poll(cx) {
                                return t;
                            }
                        }

                        if self.context.tasks.is_empty() {
                            // No task to execute, we should wait for io blockingly
                            // Hot path
                            break;
                        }

                        // Cold path
                        let _ = self.driver.submit();
                    }

                    // Wait and Process CQ(the error is ignored for not debug mode)
                    #[cfg(not(all(debug_assertions, feature = "debug")))]
                    let _ = self.driver.park();

                    #[cfg(all(debug_assertions, feature = "debug"))]
                    if let Err(e) = self.driver.park() {
                        trace!("park error: {:?}", e);
                    }
                }
            })
        })
    }
}

#[allow(unused)]
pub fn spawn<T>(future: T) -> JoinHandle<T::Output>
where
    T: Future + 'static,
    T::Output: 'static,
{
    let (task, join) = new_task(
        crate::utils::thread_id::get_current_thread_id(),
        future,
        LocalScheduler,
    );

    CURRENT.with(|ctx| {
        ctx.tasks.push(task);
    });
    join
}
// #[cfg(test)]
// mod tests {
//     use crate::builder::RuntimeBuilder;
//     use crate::driver::uring::IoUringDriver;
//
//     #[cfg(all(target_os = "linux", feature = "iouring"))]
//     #[test]
//     fn timer() {
//         let mut rt = RuntimeBuilder::<IoUringDriver>::new()
//             .enable_timer()
//             .build()
//             .unwrap();
//         let instant = std::time::Instant::now();
//         rt.block_on(async {
//             crate::time::sleep(std::time::Duration::from_millis(200)).await;
//         });
//         let eps = instant.elapsed().subsec_millis();
//         assert!((eps as i32 - 200).abs() < 50);
//     }
// }
