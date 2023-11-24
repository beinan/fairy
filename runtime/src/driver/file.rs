//! borrowed from monoio, tokio-rs/io-uring and glommio
use super::shared_fd::SharedFd;
use std::{io, path::Path};

use super::op::Op;
pub struct File {
    fd: SharedFd,
}

#[allow(dead_code)]
impl File {
    pub async fn create(path: impl AsRef<Path>) -> io::Result<File> {
        let op = Op::open(path.as_ref(), libc::O_WRONLY | libc::O_CREAT, 0o666)?;

        // Await the completion of the event
        let completion = op.await;

        // The file is open
        Ok(File::from_shared_fd(SharedFd::new_without_register(
            completion.meta.result? as _,
        )))
    }

    pub(crate) fn from_shared_fd(fd: SharedFd) -> File {
        File { fd }
    }

    pub async fn close(self) -> io::Result<()> {
        self.fd.close().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::RuntimeBuilder;
    use crate::driver::file::File;
    use crate::driver::uring::IoUringDriver;
    use tempfile::tempdir;

    #[test]
    fn test_create_file() {
        let mut rt = RuntimeBuilder::<IoUringDriver>::new()
            .with_entries(256)
            .build()
            .unwrap();
        rt.block_on(async {
            // Create a temporary directory
            let temp_dir = tempdir().expect("Failed to create a temporary directory");
            let file_path = temp_dir.path().join("hello.txt");

            let file = File::create(file_path)
                .await
                .expect("Failed to create file");
            file.close().await.expect("Failed to close file");
        });
    }
}
