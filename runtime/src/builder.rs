// borrowed from monoio, tokio-rs/io-uring and glommio

use std::{io, marker::PhantomData};

use crate::driver::uring::IoUringDriver;
use crate::runtime::Runtime;
use crate::{scoped_thread_local, utils::thread_id::gen_id};

// ===== basic builder structure definition =====

/// Runtime builder
pub struct RuntimeBuilder<D> {
    // iouring entries
    entries: Option<u32>,

    urb: io_uring::Builder,

    // blocking handle
    #[cfg(feature = "sync")]
    blocking_handle: crate::blocking::BlockingHandle,
    // driver mark
    _mark: PhantomData<D>,
}

scoped_thread_local!(pub(crate) static BUILD_THREAD_ID: usize);

impl<T> Default for RuntimeBuilder<T> {
    /// Create a default runtime builder
    #[must_use]
    fn default() -> Self {
        RuntimeBuilder::<T>::new()
    }
}

impl<T> RuntimeBuilder<T> {
    /// Create a default runtime builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: None,

            #[cfg(all(target_os = "linux", feature = "iouring"))]
            urb: io_uring::IoUring::builder(),

            #[cfg(feature = "sync")]
            blocking_handle: crate::blocking::BlockingStrategy::Panic.into(),
            _mark: PhantomData,
        }
    }
}

// ===== buildable trait and forward methods =====

/// Buildable trait.
pub trait Buildable: Sized {
    /// Build the runtime.
    fn build(this: RuntimeBuilder<Self>) -> io::Result<Runtime<Self>>;
}
impl RuntimeBuilder<IoUringDriver> {
    pub fn build(self) -> io::Result<Runtime<IoUringDriver>> {
        Buildable::build(self)
    }
}

impl Buildable for IoUringDriver {
    fn build(this: RuntimeBuilder<Self>) -> io::Result<Runtime<IoUringDriver>> {
        let thread_id = gen_id();
        #[cfg(feature = "sync")]
        let blocking_handle = this.blocking_handle;

        BUILD_THREAD_ID.set(&thread_id, || {
            let driver = match this.entries {
                Some(entries) => IoUringDriver::new_with_entries(&this.urb, entries)?,
                None => IoUringDriver::new(&this.urb)?,
            };
            #[cfg(feature = "sync")]
            let context = crate::runtime::Context::new(blocking_handle);
            #[cfg(not(feature = "sync"))]
            let context = crate::runtime::Context::new();
            Ok(Runtime::new(context, driver))
        })
    }
}

impl<D> RuntimeBuilder<D> {
    const MIN_ENTRIES: u32 = 256;

    /// Set uring entries, min size is 256 and the default size is 1024.
    #[must_use]
    pub fn with_entries(mut self, entries: u32) -> Self {
        // If entries is less than 256, it will be 256.
        if entries < Self::MIN_ENTRIES {
            self.entries = Some(Self::MIN_ENTRIES);
            return self;
        }
        self.entries = Some(entries);
        self
    }

    /// Replaces the default [`io_uring::Builder`], which controls the settings for the
    /// inner `uring` API.
    ///
    /// Refer to the [`io_uring::Builder`] documentation for all the supported methods.

    pub fn uring_builder(mut self, urb: io_uring::Builder) -> Self {
        self.urb = urb;
        self
    }
}

impl<D> RuntimeBuilder<D> {
    /// Attach thread pool, this will overwrite blocking strategy.
    /// All `spawn_blocking` will be executed on given thread pool.
    #[cfg(feature = "sync")]
    #[must_use]
    pub fn attach_thread_pool(
        mut self,
        tp: Box<dyn crate::blocking::ThreadPool + Send + 'static>,
    ) -> Self {
        self.blocking_handle = crate::blocking::BlockingHandle::Attached(tp);
        self
    }

    /// Set blocking strategy, this will overwrite thread pool setting.
    /// If `BlockingStrategy::Panic` is used, it will panic if `spawn_blocking` on this thread.
    /// If `BlockingStrategy::ExecuteLocal` is used, it will execute with current thread, and may
    /// cause tasks high latency.
    /// Attaching a thread pool is recommended if `spawn_blocking` will be used.
    #[cfg(feature = "sync")]
    #[must_use]
    pub fn with_blocking_strategy(mut self, strategy: crate::blocking::BlockingStrategy) -> Self {
        self.blocking_handle = crate::blocking::BlockingHandle::Empty(strategy);
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::RuntimeBuilder;
    use crate::driver::uring::IoUringDriver;

    #[test]
    fn test_builder() {
        let mut rt = RuntimeBuilder::<IoUringDriver>::new()
            .with_entries(256)
            .build()
            .unwrap();
        rt.block_on(async {
            println!("it works1!");
        });
    }
}
