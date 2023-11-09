use std::{
    convert::TryInto,
    mem::size_of,
    os::unix::prelude::OsStrExt,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use zerocopy::AsBytes;

use crate::uring_fuse::file_meta::{FileAttr, FileType};

use super::{kernel_interface::{fuse_attr, fuse_dirent, fuse_direntplus, fuse_entry_out}, response::Response};
use super::response::ResponseBuf;

/// Returns a fuse_attr from FileAttr
pub(crate) fn fuse_attr_from_attr(attr: &FileAttr) -> fuse_attr {
    let (atime_secs, atime_nanos) = time_from_system_time(&attr.atime);
    let (mtime_secs, mtime_nanos) = time_from_system_time(&attr.mtime);
    let (ctime_secs, ctime_nanos) = time_from_system_time(&attr.ctime);
    #[cfg(target_os = "macos")]
    let (crtime_secs, crtime_nanos) = time_from_system_time(&attr.crtime);

    fuse_attr {
        ino: attr.ino,
        size: attr.size,
        blocks: attr.blocks,
        atime: atime_secs,
        mtime: mtime_secs,
        ctime: ctime_secs,
        #[cfg(target_os = "macos")]
        crtime: crtime_secs as u64,
        atimensec: atime_nanos,
        mtimensec: mtime_nanos,
        ctimensec: ctime_nanos,
        #[cfg(target_os = "macos")]
        crtimensec: crtime_nanos,
        mode: mode_from_kind_and_perm(attr.kind, attr.perm),
        nlink: attr.nlink,
        uid: attr.uid,
        gid: attr.gid,
        rdev: attr.rdev,
        #[cfg(target_os = "macos")]
        flags: attr.flags,
        #[cfg(feature = "abi-7-9")]
        blksize: attr.blksize,
        #[cfg(feature = "abi-7-9")]
        padding: 0,
    }
}

pub(crate) fn time_from_system_time(system_time: &SystemTime) -> (i64, u32) {
    // Convert to signed 64-bit time with epoch at 0
    match system_time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs() as i64, duration.subsec_nanos()),
        Err(before_epoch_error) => (
            -(before_epoch_error.duration().as_secs() as i64),
            before_epoch_error.duration().subsec_nanos(),
        ),
    }
}

// Some platforms like Linux x86_64 have mode_t = u32, and lint warns of a trivial_numeric_casts.
// But others like macOS x86_64 have mode_t = u16, requiring a typecast.  So, just silence lint.
#[allow(trivial_numeric_casts)]
#[allow(clippy::unnecessary_cast)]
/// Returns the mode for a given file kind and permission
pub(crate) fn mode_from_kind_and_perm(kind: FileType, perm: u16) -> u32 {
    (match kind {
        FileType::NamedPipe => libc::S_IFIFO,
        FileType::CharDevice => libc::S_IFCHR,
        FileType::BlockDevice => libc::S_IFBLK,
        FileType::Directory => libc::S_IFDIR,
        FileType::RegularFile => libc::S_IFREG,
        FileType::Symlink => libc::S_IFLNK,
        FileType::Socket => libc::S_IFSOCK,
    }) as u32
        | perm as u32
}

// TODO: Add methods for creating this without making a `FileAttr` first.
#[derive(Debug, Clone, Copy)]
pub struct Attr {
    pub(crate) attr: fuse_attr,
}
impl From<&FileAttr> for Attr {
    fn from(attr: &FileAttr) -> Self {
        Self {
            attr: fuse_attr_from_attr(attr),
        }
    }
}
impl From<FileAttr> for Attr {
    fn from(attr: FileAttr) -> Self {
        Self {
            attr: fuse_attr_from_attr(&attr),
        }
    }
}

#[derive(Debug)]
pub struct EntListBuf {
    pub(super) max_size: usize,
    pub(super) buf: ResponseBuf,
}
impl EntListBuf {
    fn new(max_size: usize) -> Self {
        Self {
            max_size,
            buf: ResponseBuf::new(),
        }
    }

    /// Add an entry to the directory reply buffer. Returns true if the buffer is full.
    /// A transparent offset value can be provided for each entry. The kernel uses these
    /// value to request the next entries in further readdir calls
    #[must_use]
    fn push(&mut self, ent: [&[u8]; 2]) -> bool {
        let entlen = ent[0].len() + ent[1].len();
        let entsize = (entlen + size_of::<u64>() - 1) & !(size_of::<u64>() - 1); // 64bit align
        if self.buf.len() + entsize > self.max_size {
            return true;
        }
        self.buf.extend_from_slice(ent[0]);
        self.buf.extend_from_slice(ent[1]);
        let padlen = entsize - entlen;
        self.buf.extend_from_slice(&[0u8; 8][..padlen]);
        false
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct DirEntOffset(pub i64);
impl From<DirEntOffset> for i64 {
    fn from(x: DirEntOffset) -> Self {
        x.0
    }
}

#[derive(Debug)]
pub struct DirEntry<T: AsRef<Path>> {
    ino: u64,
    offset: DirEntOffset,
    kind: FileType,
    name: T,
}

impl<T: AsRef<Path>> DirEntry<T> {
    pub fn new(ino: u64, offset: DirEntOffset, kind: FileType, name: T) -> DirEntry<T> {
        DirEntry::<T> {
            ino,
            offset,
            kind,
            name,
        }
    }
}

/// Used to respond to [ReadDirPlus] requests.
#[derive(Debug)]
pub struct DirEntList(EntListBuf);
impl From<DirEntList> for Response<'_> {
    fn from(l: DirEntList) -> Self {
        assert!(l.0.buf.len() <= l.0.max_size);
        Response::new_directory(l.0)
    }
}

impl DirEntList {
    pub(crate) fn new(max_size: usize) -> Self {
        Self(EntListBuf::new(max_size))
    }
    /// Add an entry to the directory reply buffer. Returns true if the buffer is full.
    /// A transparent offset value can be provided for each entry. The kernel uses these
    /// value to request the next entries in further readdir calls
    #[must_use]
    pub fn push<T: AsRef<Path>>(&mut self, ent: &DirEntry<T>) -> bool {
        let name = ent.name.as_ref().as_os_str().as_bytes();
        let header = fuse_dirent {
            ino: ent.ino.into(),
            off: ent.offset.0,
            namelen: name.len().try_into().expect("Name too long"),
            typ: mode_from_kind_and_perm(ent.kind, 0) >> 12,
        };
        self.0.push([header.as_bytes(), name])
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct DirEntryPlus<T: AsRef<Path>> {
    #[allow(unused)] // We use `attr.ino` instead
    ino: u64,
    generation: u64,
    offset: DirEntOffset,
    name: T,
    entry_valid: Duration,
    attr: Attr,
    attr_valid: Duration,
}

#[allow(dead_code)]
impl<T: AsRef<Path>> DirEntryPlus<T> {
    pub fn new(
        ino: u64,
        generation: u64,
        offset: DirEntOffset,
        name: T,
        entry_valid: Duration,
        attr: Attr,
        attr_valid: Duration,
    ) -> Self {
        Self {
            ino,
            generation,
            offset,
            name,
            entry_valid,
            attr,
            attr_valid,
        }
    }
}

/// Used to respond to [ReadDir] requests.
#[derive(Debug)]
pub struct DirEntPlusList(EntListBuf);
impl From<DirEntPlusList> for Response<'_> {
    fn from(l: DirEntPlusList) -> Self {
        assert!(l.0.buf.len() <= l.0.max_size);
        Response::new_directory(l.0)
    }
}

#[allow(dead_code)]
impl DirEntPlusList {
    pub(crate) fn new(max_size: usize) -> Self {
        Self(EntListBuf::new(max_size))
    }
    /// Add an entry to the directory reply buffer. Returns true if the buffer is full.
    /// A transparent offset value can be provided for each entry. The kernel uses these
    /// value to request the next entries in further readdir calls
    #[must_use]
    pub fn push<T: AsRef<Path>>(&mut self, x: &DirEntryPlus<T>) -> bool {
        let name = x.name.as_ref().as_os_str().as_bytes();
        let header = fuse_direntplus {
            entry_out: fuse_entry_out {
                nodeid: x.attr.attr.ino,
                generation: x.generation.into(),
                entry_valid: x.entry_valid.as_secs(),
                attr_valid: x.attr_valid.as_secs(),
                entry_valid_nsec: x.entry_valid.subsec_nanos(),
                attr_valid_nsec: x.attr_valid.subsec_nanos(),
                attr: x.attr.attr,
            },
            dirent: fuse_dirent {
                ino: x.attr.attr.ino,
                off: x.offset.into(),
                namelen: name.len().try_into().expect("Name too long"),
                typ: x.attr.attr.mode >> 12,
            },
        };
        self.0.push([header.as_bytes(), name])
    }
}