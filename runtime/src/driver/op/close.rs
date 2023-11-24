use std::io;
#[cfg(unix)]
use std::os::unix::io::RawFd;

use super::{Op, OpAble};
#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};

pub(crate) struct Close {
    #[cfg(unix)]
    fd: RawFd,
}

impl Op<Close> {
    #[allow(unused)]
    #[cfg(unix)]
    pub(crate) fn close(fd: RawFd) -> io::Result<Op<Close>> {
        Op::try_submit_with(Close { fd })
    }
}

impl OpAble for Close {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::Close::new(types::Fd(self.fd)).build()
    }
}
