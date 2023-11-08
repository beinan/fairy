use super::{kernel_interface as abi, lock::Lock, errno::Errno, file_meta::{Attr, EntListBuf}};

use std::{
    convert::TryInto,
    io::IoSlice,
    mem::size_of,
    time::{Duration, SystemTime},
};

use smallvec::{smallvec, SmallVec};
use zerocopy::AsBytes;

const INLINE_DATA_THRESHOLD: usize = size_of::<u64>() * 4;
pub(crate) type ResponseBuf = SmallVec<[u8; INLINE_DATA_THRESHOLD]>;

pub enum Response<'a> {
    Error(i32),
    Data(ResponseBuf),
    Slice(&'a [u8]),
}

#[allow(dead_code)]
impl<'a> Response<'a> {
    pub(crate) fn with_iovec<F: FnOnce(&[IoSlice<'_>]) -> T, T>(
        &self,
        unique: u64,
        f: F,
    ) -> T {
        let datalen = match &self {
            Response::Error(_) => 0,
            Response::Data(v) => v.len(),
            Response::Slice(d) => d.len(),
        };
        let header = abi::fuse_out_header {
            unique: unique,
            error: if let Response::Error(errno) = self {
                -errno
            } else {
                0
            },
            len: (size_of::<abi::fuse_out_header>() + datalen)
                .try_into()
                .expect("Too much data"),
        };
        let mut v: SmallVec<[IoSlice<'_>; 3]> = smallvec![IoSlice::new(header.as_bytes())];
        match &self {
            Response::Error(_) => {}
            Response::Data(d) => v.push(IoSlice::new(d)),
            Response::Slice(d) => v.push(IoSlice::new(d)),
        }
        f(&v)
    }

    // Constructors
    pub(crate) fn new_empty() -> Self {
        Self::Error(0)
    }

    pub(crate) fn new_error(error: Errno) -> Self {
        Self::Error(error.into())
    }

    pub(crate) fn new_data<T: AsRef<[u8]> + Into<Vec<u8>>>(data: T) -> Self {
        Self::Data(if data.as_ref().len() <= INLINE_DATA_THRESHOLD {
            data.as_ref().into()
        } else {
            data.into().into()
        })
    }

    pub(crate) fn new_slice(data: &'a [u8]) -> Self {
        Self::Slice(data)
    }

    pub(crate) fn new_entry(
        ino: u64,
        generation: u64,
        attr: &Attr,
        attr_ttl: Duration,
        entry_ttl: Duration,
    ) -> Self {
        let d = abi::fuse_entry_out {
            nodeid: ino.into(),
            generation: generation.into(),
            entry_valid: entry_ttl.as_secs(),
            attr_valid: attr_ttl.as_secs(),
            entry_valid_nsec: entry_ttl.subsec_nanos(),
            attr_valid_nsec: attr_ttl.subsec_nanos(),
            attr: attr.attr,
        };
        Self::from_struct(d.as_bytes())
    }

    pub(crate) fn new_attr(ttl: &Duration, attr: &Attr) -> Self {
        let r = abi::fuse_attr_out {
            attr_valid: ttl.as_secs(),
            attr_valid_nsec: ttl.subsec_nanos(),
            dummy: 0,
            attr: attr.attr,
        };
        Self::from_struct(&r)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn new_xtimes(bkuptime: SystemTime, crtime: SystemTime) -> Self {
        use super::file_meta::time_from_system_time;


        let (bkuptime_secs, bkuptime_nanos) = time_from_system_time(&bkuptime);
        let (crtime_secs, crtime_nanos) = time_from_system_time(&crtime);
        let r = abi::fuse_getxtimes_out {
            bkuptime: bkuptime_secs as u64,
            crtime: crtime_secs as u64,
            bkuptimensec: bkuptime_nanos,
            crtimensec: crtime_nanos,
        };
        Self::from_struct(&r)
    }

    // TODO: Could flags be more strongly typed?
    pub(crate) fn new_open(file_handle: u64, flags: u32) -> Self {
        let r = abi::fuse_open_out {
            fh: file_handle,
            open_flags: flags,
            padding: 0,
        };
        Self::from_struct(&r)
    }

    pub(crate) fn new_lock(lock: &Lock) -> Self {
        let r = abi::fuse_lk_out {
            lk: abi::fuse_file_lock {
                start: lock.range.0,
                end: lock.range.1,
                typ: lock.typ,
                pid: lock.pid,
            },
        };
        Self::from_struct(&r)
    }

    pub(crate) fn new_bmap(block: u64) -> Self {
        let r = abi::fuse_bmap_out { block };
        Self::from_struct(&r)
    }

    pub(crate) fn new_write(written: u32) -> Self {
        let r = abi::fuse_write_out {
            size: written,
            padding: 0,
        };
        Self::from_struct(&r)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_statfs(
        blocks: u64,
        bfree: u64,
        bavail: u64,
        files: u64,
        ffree: u64,
        bsize: u32,
        namelen: u32,
        frsize: u32,
    ) -> Self {
        let r = abi::fuse_statfs_out {
            st: abi::fuse_kstatfs {
                blocks,
                bfree,
                bavail,
                files,
                ffree,
                bsize,
                namelen,
                frsize,
                padding: 0,
                spare: [0; 6],
            },
        };
        Self::from_struct(&r)
    }

    // TODO: Can flags be more strongly typed?
    pub(crate) fn new_create(
        ttl: &Duration,
        attr: &Attr,
        generation: u64,
        file_handle: u64,
        flags: u32,
    ) -> Self {
        let r = abi::fuse_create_out(
            abi::fuse_entry_out {
                nodeid: attr.attr.ino,
                generation: generation,
                entry_valid: ttl.as_secs(),
                attr_valid: ttl.as_secs(),
                entry_valid_nsec: ttl.subsec_nanos(),
                attr_valid_nsec: ttl.subsec_nanos(),
                attr: attr.attr,
            },
            abi::fuse_open_out {
                fh: file_handle,
                open_flags: flags,
                padding: 0,
            },
        );
        Self::from_struct(&r)
    }

    // TODO: Are you allowed to send data while result != 0?
    pub(crate) fn new_ioctl(result: i32, data: &[IoSlice<'_>]) -> Self {
        let r = abi::fuse_ioctl_out {
            result,
            // these fields are only needed for unrestricted ioctls
            flags: 0,
            in_iovs: 1,
            out_iovs: if !data.is_empty() { 1 } else { 0 },
        };
        // TODO: Don't copy this data
        let mut v: ResponseBuf = r.as_bytes().into();
        for x in data {
            v.extend_from_slice(x)
        }
        Self::Data(v)
    }

    pub(crate) fn new_directory(list: EntListBuf) -> Self {
        assert!(list.buf.len() <= list.max_size);
        Self::Data(list.buf)
    }

    pub(crate) fn new_xattr_size(size: u32) -> Self {
        let r = abi::fuse_getxattr_out { size, padding: 0 };
        Self::from_struct(&r)
    }

    pub(crate) fn new_lseek(offset: i64) -> Self {
        let r = abi::fuse_lseek_out { offset };
        Self::from_struct(&r)
    }

    fn from_struct<T: AsBytes + ?Sized>(data: &T) -> Self {
        Self::Data(data.as_bytes().into())
    }
}
