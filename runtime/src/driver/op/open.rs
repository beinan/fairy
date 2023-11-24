use super::{Op, OpAble};
use io_uring::opcode::OpenAt;
use io_uring::squeue::Entry;
use io_uring::types;
use libc::mode_t;
use std::{ffi::CString, io, path::Path};

pub(crate) struct Open {
    pub(crate) path: CString,
    flags: i32,
    mode: mode_t,
}
impl Op<Open> {
    pub(crate) fn open<P: AsRef<Path>>(path: P, flags: i32, mode: mode_t) -> io::Result<Op<Open>> {
        // Here the path will be copied, so its safe.
        let path = cstr(path.as_ref())?;
        Op::submit_with(Open { path, flags, mode })
    }
}

impl OpAble for Open {
    fn uring_op(&mut self) -> Entry {
        OpenAt::new(types::Fd(libc::AT_FDCWD), self.path.as_c_str().as_ptr())
            .flags(self.flags)
            .mode(self.mode)
            .build()
    }
}
fn cstr(p: &Path) -> io::Result<CString> {
    use std::os::unix::ffi::OsStrExt;
    Ok(CString::new(p.as_os_str().as_bytes())?)
}
