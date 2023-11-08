use super::lock::Lock;
use super::response::Response;
use super::lock::LockOwner;
use super::operation::Operation;
use super::argument::ArgumentIterator;
use super::kernel_interface::*;
use super::kernel_interface::consts::*;
use super::request::{Request,RequestError};

use crate::uring_fuse::{TimeOrNow, KernelConfig};
use std::{
    convert::TryInto,
    ffi::OsStr,
    fmt::{Display, self},
    num::NonZeroU32,
    path::Path,
    time::{Duration, SystemTime}, mem,
};
use zerocopy::AsBytes;


macro_rules! impl_request {
    ($structname: ty) => {
        impl<'a> Request for $structname {
            #[inline]
            fn unique(&self) -> u64 {
                self.header.unique                                                                                                                  
            }

            #[inline]
            fn nodeid(&self) -> u64 {
                self.header.nodeid
            }

            #[inline]
            fn uid(&self) -> u32 {
                self.header.uid
            }

            #[inline]
            fn gid(&self) -> u32 {
                self.header.gid
            }

            #[inline]
            fn pid(&self) -> u32 {
                self.header.pid
            }
        }
    };
}

/// Low-level request of a filesystem operation the kernel driver wants to perform.
#[derive(Debug)]
pub struct AnyRequest<'a> {
    header: &'a fuse_in_header,
    data: &'a [u8],
}

impl<'a> AnyRequest<'a> {
    pub fn operation(&self) -> Result<Operation<'a>, RequestError> {
        // Parse/check opcode
        let opcode = fuse_opcode::try_from(self.header.opcode)
            .map_err(|_: InvalidOpcodeError| RequestError::UnknownOperation(self.header.opcode))?;
        // Parse/check operation arguments
        parse(self.header, &opcode, self.data).ok_or(RequestError::InsufficientData)
    }
}

impl_request!(AnyRequest<'_>);

impl<'a> fmt::Display for AnyRequest<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(op) = self.operation() {
            write!(
                f,
                "FUSE({:3}) ino {:#018x} {}",
                self.header.unique, self.header.nodeid, op
            )
        } else {
            write!(
                f,
                "FUSE({:3}) ino {:#018x}",
                self.header.unique, self.header.nodeid
            )
        }
    }
}

impl<'a> TryFrom<&'a [u8]> for AnyRequest<'a> {
    type Error = RequestError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        // Parse a raw packet as sent by the kernel driver into typed data. Every request always
        // begins with a `fuse_in_header` struct followed by arguments depending on the opcode.
        let data_len = data.len();
        let mut arg_iter = ArgumentIterator::new(data);
        // Parse header
        let header: &fuse_in_header = arg_iter
            .fetch()
            .ok_or_else(|| RequestError::ShortReadHeader(arg_iter.len()))?;
        // Check data size
        if data_len < header.len as usize {
            return Err(RequestError::ShortRead(data_len, header.len as usize));
        }
        Ok(Self {
            header,
            data: &data[mem::size_of::<fuse_in_header>()..header.len as usize],
        })
    }
}

/// Represents a filename in a directory
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct FilenameInDir<'a> {
    /// The Inode number of the directory
    pub dir: u64,
    /// Name of the file. This refers to a name directly in this directory, rather than any
    /// subdirectory so is guaranteed not to contain '\0' or '/'.  It may be literally "." or ".."
    /// however.
    pub name: &'a Path,
}

/// Look up a directory entry by name and get its attributes.
///
/// Implementations allocate and assign [INodeNo]s in this request.  Learn more
/// about INode lifecycle and the relationship between [Lookup] and [Forget] in the
/// documentation for [INodeNo].
#[derive(Debug)]
pub struct Lookup<'a> {
    header: &'a fuse_in_header,
    name: &'a OsStr,
}
impl_request!(Lookup<'_>);
impl<'a> Lookup<'a> {
    pub fn name(&self) -> &'a Path {
        self.name.as_ref()
    }
}
/// Forget about an inode.
///
/// The nlookup parameter indicates the number of lookups previously performed on
/// this inode. If the filesystem implements inode lifetimes, it is recommended that
/// inodes acquire a single reference on each lookup, and lose nlookup references on
/// each forget. The filesystem may ignore forget calls, if the inodes don't need to
/// have a limited lifetime.
///
/// Learn more about INode lifecycle in the documentation for [INodeNo].
///
/// On unmount it is not guaranteed, that all referenced inodes will receive a forget
/// message.
#[derive(Debug)]
pub struct Forget<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_forget_in,
}
impl_request!(Forget<'_>);
impl<'a> Forget<'a> {
    /// The number of lookups previously performed on this inode
    pub fn nlookup(&self) -> u64 {
        self.arg.nlookup
    }
}

/// Get file attributes.
#[derive(Debug)]
pub struct GetAttr<'a> {
    header: &'a fuse_in_header,
}
impl_request!(GetAttr<'_>);

/// Set file attributes.
#[derive(Debug)]
pub struct SetAttr<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_setattr_in,
}
impl_request!(SetAttr<'_>);
impl<'a> SetAttr<'a> {
    pub fn mode(&self) -> Option<u32> {
        match self.arg.valid & FATTR_MODE {
            0 => None,
            _ => Some(self.arg.mode),
        }
    }
    pub fn uid(&self) -> Option<u32> {
        match self.arg.valid & FATTR_UID {
            0 => None,
            _ => Some(self.arg.uid),
        }
    }
    pub fn gid(&self) -> Option<u32> {
        match self.arg.valid & FATTR_GID {
            0 => None,
            _ => Some(self.arg.gid),
        }
    }
    pub fn size(&self) -> Option<u64> {
        match self.arg.valid & FATTR_SIZE {
            0 => None,
            _ => Some(self.arg.size),
        }
    }
    pub fn atime(&self) -> Option<TimeOrNow> {
        match self.arg.valid & FATTR_ATIME {
            0 => None,
            _ => Some(if self.arg.atime_now() {
                TimeOrNow::Now
            } else {
                TimeOrNow::SpecificTime(system_time_from_time(
                    self.arg.atime,
                    self.arg.atimensec,
                ))
            }),
        }
    }
    pub fn mtime(&self) -> Option<TimeOrNow> {
        match self.arg.valid & FATTR_MTIME {
            0 => None,
            _ => Some(if self.arg.mtime_now() {
                TimeOrNow::Now
            } else {
                TimeOrNow::SpecificTime(system_time_from_time(
                    self.arg.mtime,
                    self.arg.mtimensec,
                ))
            }),
        }
    }
    pub fn ctime(&self) -> Option<SystemTime> {
        #[cfg(feature = "abi-7-23")]
        match self.arg.valid & FATTR_CTIME {
            0 => None,
            _ => Some(system_time_from_time(self.arg.ctime, self.arg.ctimensec)),
        }
        #[cfg(not(feature = "abi-7-23"))]
        None
    }
    /// The value set by the [Open] method. See [FileHandle].
    ///
    /// This will only be set if the user passed a file-descriptor to set the
    /// attributes - i.e. they used [libc::fchmod] rather than [libc::chmod].
    pub fn file_handle(&self) -> Option<u64> {
        match self.arg.valid & FATTR_FH {
            0 => None,
            _ => Some(self.arg.fh),
        }
    }
    pub fn crtime(&self) -> Option<SystemTime> {
        #[cfg(target_os = "macos")]
        match self.arg.valid & FATTR_CRTIME {
            0 => None,
            _ => Some(
                SystemTime::UNIX_EPOCH + Duration::new(self.arg.crtime, self.arg.crtimensec),
            ),
        }
        #[cfg(not(target_os = "macos"))]
        None
    }
    pub fn chgtime(&self) -> Option<SystemTime> {
        #[cfg(target_os = "macos")]
        match self.arg.valid & FATTR_CHGTIME {
            0 => None,
            _ => Some(
                SystemTime::UNIX_EPOCH + Duration::new(self.arg.chgtime, self.arg.chgtimensec),
            ),
        }
        #[cfg(not(target_os = "macos"))]
        None
    }
    pub fn bkuptime(&self) -> Option<SystemTime> {
        #[cfg(target_os = "macos")]
        match self.arg.valid & FATTR_BKUPTIME {
            0 => None,
            _ => Some(
                SystemTime::UNIX_EPOCH
                    + Duration::new(self.arg.bkuptime, self.arg.bkuptimensec),
            ),
        }
        #[cfg(not(target_os = "macos"))]
        None
    }
    pub fn flags(&self) -> Option<u32> {
        #[cfg(target_os = "macos")]
        match self.arg.valid & FATTR_FLAGS {
            0 => None,
            _ => Some(self.arg.flags),
        }
        #[cfg(not(target_os = "macos"))]
        None
    }

    // TODO: Why does *set*attr want to have an attr response?
}
impl<'a> Display for SetAttr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SETATTR mode: {:?}, uid: {:?}, gid: {:?}, size: {:?}, atime: {:?}, \
            mtime: {:?}, ctime: {:?}, file_handle: {:?}, crtime: {:?}, chgtime: {:?}, \
            bkuptime: {:?}, flags: {:?}",
            self.mode(),
            self.uid(),
            self.gid(),
            self.size(),
            self.atime(),
            self.mtime(),
            self.ctime(),
            self.file_handle(),
            self.crtime(),
            self.chgtime(),
            self.bkuptime(),
            self.flags()
        )
    }
}

/// Read symbolic link.
#[derive(Debug)]
pub struct ReadLink<'a> {
    header: &'a fuse_in_header,
}
impl_request!(ReadLink<'_>);

/// Create a symbolic link.
#[derive(Debug)]
pub struct SymLink<'a> {
    header: &'a fuse_in_header,
    target: &'a Path,
    link: &'a Path,
}
impl_request!(SymLink<'_>);
impl<'a> SymLink<'a> {
    pub fn target(&self) -> &'a Path {
        self.target
    }
    pub fn link(&self) -> &'a Path {
        self.link
    }
}

/// Create file node.
/// Create a regular file, character device, block device, fifo or socket node.
#[derive(Debug)]
pub struct MkNod<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_mknod_in,
    name: &'a Path,
}
impl_request!(MkNod<'_>);
impl<'a> MkNod<'a> {
    pub fn name(&self) -> &'a Path {
        self.name
    }
    pub fn mode(&self) -> u32 {
        self.arg.mode
    }
    pub fn umask(&self) -> u32 {
        #[cfg(not(feature = "abi-7-12"))]
        return 0;
        #[cfg(feature = "abi-7-12")]
        self.arg.umask
    }
    pub fn rdev(&self) -> u32 {
        self.arg.rdev
    }
}

/// Create a directory.
#[derive(Debug)]
pub struct MkDir<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_mkdir_in,
    name: &'a Path,
}
impl_request!(MkDir<'_>);
impl<'a> MkDir<'a> {
    pub fn name(&self) -> &'a Path {
        self.name
    }
    pub fn mode(&self) -> u32 {
        self.arg.mode
    }
    pub fn umask(&self) -> u32 {
        #[cfg(not(feature = "abi-7-12"))]
        return 0;
        #[cfg(feature = "abi-7-12")]
        self.arg.umask
    }
}

/// Remove a file.
#[derive(Debug)]
pub struct Unlink<'a> {
    header: &'a fuse_in_header,
    name: &'a Path,
}
impl_request!(Unlink<'_>);
impl<'a> Unlink<'a> {
    pub fn name(&self) -> &'a Path {
        self.name
    }
}

/// Remove a directory.
#[derive(Debug)]
pub struct RmDir<'a> {
    header: &'a fuse_in_header,
    pub name: &'a Path,
}
impl_request!(RmDir<'_>);
impl<'a> RmDir<'a> {
    pub fn name(&self) -> &'a Path {
        self.name
    }
}

/// Rename a file.
#[derive(Debug)]
pub struct Rename<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_rename_in,
    name: &'a Path,
    newname: &'a Path,
}
impl_request!(Rename<'_>);
impl<'a> Rename<'a> {
    pub fn src(&self) -> FilenameInDir<'a> {
        FilenameInDir::<'a> {
            dir: self.header.nodeid,
            name: self.name,
        }
    }
    pub fn dest(&self) -> FilenameInDir<'a> {
        FilenameInDir::<'a> {
            dir: self.arg.newdir,
            name: self.newname,
        }
    }
}

/// Create a hard link.
#[derive(Debug)]
pub struct Link<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_link_in,
    name: &'a Path,
}
impl_request!(Link<'_>);
impl<'a> Link<'a> {
    /// This is the inode no of the file to be linked.  The inode number in
    /// the fuse header is of the directory that it will be linked into.
    pub fn inode_no(&self) -> u64 {
        self.arg.oldnodeid
    }
    pub fn dest(&self) -> FilenameInDir<'a> {
        FilenameInDir::<'a> {
            dir: self.inode_no(),
            name: self.name,
        }
    }
}

/// Open a file.
///
/// Open flags (with the exception of `O_CREAT`, `O_EXCL`, `O_NOCTTY` and `O_TRUNC`) are
/// available in flags. Filesystem may store an arbitrary file handle (pointer, index,
/// etc) in fh, and use this in other all other file operations (read, write, flush,
/// release, fsync). Filesystem may also implement stateless file I/O and not store
/// anything in fh. There are also some flags (direct_io, keep_cache) which the
/// filesystem may set, to change the way the file is opened. See fuse_file_info
/// structure in <fuse_common.h> for more details.
#[derive(Debug)]
pub struct Open<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_open_in,
}
impl_request!(Open<'_>);
impl<'a> Open<'a> {
    pub fn flags(&self) -> i32 {
        self.arg.flags
    }
}

/// Read data.
///
/// Read should send exactly the number of bytes requested except on EOF or error,
/// otherwise the rest of the data will be substituted with zeroes. An exception to
/// this is when the file has been opened in 'direct_io' mode, in which case the
/// return value of the read system call will reflect the return value of this
/// operation.
#[derive(Debug)]
pub struct Read<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_read_in,
}
impl_request!(Read<'_>);
impl<'a> Read<'a> {
    /// The value set by the [Open] method.
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn offset(&self) -> i64 {
        self.arg.offset
    }
    pub fn size(&self) -> u32 {
        self.arg.size
    }
    /// Only supported with ABI >= 7.9
    pub fn lock_owner(&self) -> Option<LockOwner> {
        #[cfg(not(feature = "abi-7-9"))]
        return None;
        #[cfg(feature = "abi-7-9")]
        if self.arg.read_flags & FUSE_READ_LOCKOWNER != 0 {
            Some(LockOwner(self.arg.lock_owner))
        } else {
            None
        }
    }
    /// The file flags, such as `O_SYNC`. Only supported with ABI >= 7.9
    pub fn flags(&self) -> i32 {
        #[cfg(not(feature = "abi-7-9"))]
        return 0;
        #[cfg(feature = "abi-7-9")]
        self.arg.flags
    }
}

/// Write data.
///
/// Write should return exactly the number of bytes requested except on error. An
/// exception to this is when the file has been opened in 'direct_io' mode, in
/// which case the return value of the write system call will reflect the return
/// value of this operation.
#[derive(Debug)]
pub struct Write<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_write_in,
    data: &'a [u8],
}
impl_request!(Write<'_>);
impl<'a> Write<'a> {
    /// The value set by the [Open] method.
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn offset(&self) -> i64 {
        self.arg.offset
    }
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
    /// Will contain FUSE_WRITE_CACHE, if this write is from the page cache. If set,
    /// the pid, uid, gid, and fh may not match the value that would have been sent if write caching
    /// is disabled
    ///
    /// TODO: WriteFlags type or remove this
    pub fn write_flags(&self) -> u32 {
        self.arg.write_flags
    }
    /// lock_owner: only supported with ABI >= 7.9
    pub fn lock_owner(&self) -> Option<LockOwner> {
        #[cfg(feature = "abi-7-9")]
        if self.arg.write_flags & FUSE_WRITE_LOCKOWNER != 0 {
            Some(LockOwner(self.arg.lock_owner))
        } else {
            None
        }
        #[cfg(not(feature = "abi-7-9"))]
        None
    }
    /// flags: these are the file flags, such as O_SYNC. Only supported with ABI >= 7.9
    /// TODO: Make a Flags type specifying valid values
    pub fn flags(&self) -> i32 {
        #[cfg(feature = "abi-7-9")]
        return self.arg.flags;
        #[cfg(not(feature = "abi-7-9"))]
        0
    }
}

/// Get file system statistics.
#[derive(Debug)]
pub struct StatFs<'a> {
    header: &'a fuse_in_header,
}
impl_request!(StatFs<'_>);

/// Release an open file.
///
/// Release is called when there are no more references to an open file: all file
/// descriptors are closed and all memory mappings are unmapped. For every [Open]
/// call there will be exactly one release call. The filesystem may reply with an
/// error, but error values are not returned to `close()` or `munmap()` which
/// triggered the release.
#[derive(Debug)]
pub struct Release<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_release_in,
}
impl_request!(Release<'_>);
impl<'a> Release<'a> {
    pub fn flush(&self) -> bool {
        self.arg.release_flags & FUSE_RELEASE_FLUSH != 0
    }
    /// The value set by the [Open] method.
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    /// The same flags as for open.
    /// TODO: Document what flags are valid, or remove this
    pub fn flags(&self) -> i32 {
        self.arg.flags
    }
    pub fn lock_owner(&self) -> Option<LockOwner> {
        #[cfg(not(feature = "abi-7-17"))]
        return Some(LockOwner(self.arg.lock_owner));
        #[cfg(feature = "abi-7-17")]
        if self.arg.release_flags & FUSE_RELEASE_FLOCK_UNLOCK != 0 {
            Some(LockOwner(self.arg.lock_owner))
        } else {
            None
        }
    }
}

/// Synchronize file contents.
#[derive(Debug)]
pub struct FSync<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_fsync_in,
}
impl_request!(FSync<'a>);
impl<'a> FSync<'a> {
    /// The value set by the [Open] method.
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    /// If set only the user data should be flushed, not the meta data.
    pub fn fdatasync(&self) -> bool {
        self.arg.fsync_flags & consts::FUSE_FSYNC_FDATASYNC != 0
    }
}

/// Set an extended attribute.
#[derive(Debug)]
pub struct SetXAttr<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_setxattr_in,
    name: &'a OsStr,
    value: &'a [u8],
}
impl_request!(SetXAttr<'a>);
impl<'a> SetXAttr<'a> {
    pub fn name(&self) -> &'a OsStr {
        self.name
    }
    pub fn value(&self) -> &'a [u8] {
        self.value
    }
    // TODO: Document what are valid flags
    pub fn flags(&self) -> i32 {
        self.arg.flags
    }
    /// This will always be 0 except on MacOS.  It's recommended that
    /// implementations return EINVAL if this is not 0.
    pub fn position(&self) -> u32 {
        #[cfg(target_os = "macos")]
        return self.arg.position;
        #[cfg(not(target_os = "macos"))]
        0
    }
}

/// Get an extended attribute.
///
/// If the requested XAttr doesn't exist return [Err(Errno::NO_XATTR)] which will
/// map to the right platform-specific error code.
#[derive(Debug)]
pub struct GetXAttr<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_getxattr_in,
    name: &'a OsStr,
}
impl_request!(GetXAttr<'a>);

/// Type for [GetXAttrSizeEnum::GetSize].
///
/// Represents a request from the user to get the size of the data stored in the XAttr.
#[derive(Debug)]
pub struct GetXAttrSize();

#[derive(Debug)]
/// Return type for [GetXAttr::size].
pub enum GetXAttrSizeEnum {
    /// User is requesting the size of the data stored in the XAttr
    GetSize(GetXAttrSize),
    /// User is requesting the data stored in the XAttr.  If the data will fit
    /// in this number of bytes it should be returned, otherwise return [Err(Errno::ERANGE)].
    Size(NonZeroU32),
}
impl<'a> GetXAttr<'a> {
    /// Name of the XAttr
    pub fn name(&self) -> &'a OsStr {
        self.name
    }
    /// See [GetXAttrSizeEnum].
    ///
    /// You only need to check this value as an optimisation where there's a
    /// cost difference between checking the size of the data stored in an XAttr
    /// and actually providing the data.  Otherwise just call [reply()] with the
    /// data and it will do the right thing.
    pub fn size(&self) -> GetXAttrSizeEnum {
        let s: Result<NonZeroU32, _> = self.arg.size.try_into();
        match s {
            Ok(s) => GetXAttrSizeEnum::Size(s),
            Err(_) => GetXAttrSizeEnum::GetSize(GetXAttrSize()),
        }
    }
    /// The size of the buffer the user has allocated to store the XAttr value.
    pub(crate) fn size_u32(&self) -> u32 {
        self.arg.size
    }
}

/// List extended attribute names.
#[derive(Debug)]
pub struct ListXAttr<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_getxattr_in,
}
impl_request!(ListXAttr<'a>);
impl<'a> ListXAttr<'a> {
    /// The size of the buffer the caller has allocated to receive the list of
    /// XAttrs.  If this is 0 the user is just probing to find how much space is
    /// required to fit the whole list.
    ///
    /// You don't need to worry about this except as an optimisation.
    pub fn size(&self) -> u32 {
        self.arg.size
    }
}

/// Remove an extended attribute.
///
/// Return [Err(Errno::NO_XATTR)] if the xattr doesn't exist
/// Return [Err(Errno::ENOTSUP)] if this filesystem doesn't support XAttrs
#[derive(Debug)]
pub struct RemoveXAttr<'a> {
    header: &'a fuse_in_header,
    name: &'a OsStr,
}
impl_request!(RemoveXAttr<'a>);
impl<'a> RemoveXAttr<'a> {
    /// Name of the XAttr to remove
    pub fn name(&self) -> &'a OsStr {
        self.name
    }
}

/// Flush method.
///
/// This is called on each close() of the opened file. Since file descriptors can
/// be duplicated (dup, dup2, fork), for one open call there may be many flush
/// calls. Filesystems shouldn't assume that flush will always be called after some
/// writes, or that if will be called at all.
///
/// NOTE: the name of the method is misleading, since (unlike fsync) the filesystem
/// is not forced to flush pending writes. One reason to flush data, is if the
/// filesystem wants to return write errors. If the filesystem supports file locking
/// operations (setlk, getlk) it should remove all locks belonging to 'lock_owner'.
#[derive(Debug)]
pub struct Flush<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_flush_in,
}
impl_request!(Flush<'a>);
impl<'a> Flush<'a> {
    /// The value set by the open method
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn lock_owner(&self) -> LockOwner {
        LockOwner(self.arg.lock_owner)
    }
}

#[derive(Debug)]
pub struct Init<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_init_in,
}
impl_request!(Init<'a>);
impl<'a> Init<'a> {
    pub fn capabilities(&self) -> u32 {
        self.arg.flags
    }
    pub fn max_readahead(&self) -> u32 {
        self.arg.max_readahead
    }
    pub fn version(&self) -> super::version::Version {
        super::version::Version(self.arg.major, self.arg.minor)
    }

    pub fn reply(&self, config: &KernelConfig) -> Response<'a> {
        let init = fuse_init_out {
            major: FUSE_KERNEL_VERSION,
            minor: FUSE_KERNEL_MINOR_VERSION,
            max_readahead: config.max_readahead,
            flags: self.capabilities() & config.requested, // use requested features and reported as capable
            #[cfg(not(feature = "abi-7-13"))]
            unused: 0,
            #[cfg(feature = "abi-7-13")]
            max_background: config.max_background,
            #[cfg(feature = "abi-7-13")]
            congestion_threshold: config.congestion_threshold(),
            max_write: config.max_write,
            #[cfg(feature = "abi-7-23")]
            time_gran: config.time_gran.as_nanos() as u32,
            #[cfg(all(feature = "abi-7-23", not(feature = "abi-7-28")))]
            reserved: [0; 9],
            #[cfg(feature = "abi-7-28")]
            max_pages: config.max_pages(),
            #[cfg(feature = "abi-7-28")]
            unused2: 0,
            #[cfg(feature = "abi-7-28")]
            reserved: [0; 8],
        };
        Response::new_data(init.as_bytes())
    }
}

/// Open a directory.
///
/// Filesystem may store an arbitrary file handle (pointer, index, etc) in fh, and
/// use this in other all other directory stream operations ([ReadDir], [ReleaseDir],
/// [FSyncDir]). Filesystem may also implement stateless directory I/O and not store
/// anything in fh, though that makes it impossible to implement standard conforming
/// directory stream operations in case the contents of the directory can change
/// between [OpenDir] and [ReleaseDir].
///
/// TODO: Document how to implement "standard conforming directory stream operations"
#[derive(Debug)]
pub struct OpenDir<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_open_in,
}
impl_request!(OpenDir<'a>);
impl<'a> OpenDir<'a> {
    /// Flags as passed to open
    pub fn flags(&self) -> i32 {
        self.arg.flags
    }
}

/// Read directory.
#[derive(Debug)]
pub struct ReadDir<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_read_in,
}
impl_request!(ReadDir<'a>);
impl<'a> ReadDir<'a> {
    /// The value set by the [OpenDir] method.
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn offset(&self) -> i64 {
        self.arg.offset
    }
    pub fn size(&self) -> u32 {
        self.arg.size
    }
}

/// Release an open directory.
///
/// For every [OpenDir] call there will be exactly one [ReleaseDir] call.
#[derive(Debug)]
pub struct ReleaseDir<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_release_in,
}
impl_request!(ReleaseDir<'a>);
impl<'a> ReleaseDir<'a> {
    /// The value set by the [OpenDir] method.
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn flush(&self) -> bool {
        self.arg.release_flags & consts::FUSE_RELEASE_FLUSH != 0
    }
    pub fn lock_owner(&self) -> Option<LockOwner> {
        #[cfg(not(feature = "abi-7-17"))]
        return Some(LockOwner(self.arg.lock_owner));
        #[cfg(feature = "abi-7-17")]
        if self.arg.release_flags & FUSE_RELEASE_FLOCK_UNLOCK != 0 {
            Some(LockOwner(self.arg.lock_owner))
        } else {
            None
        }
    }
    /// TODO: Document what values this may take
    pub fn flags(&self) -> i32 {
        self.arg.flags
    }
}

/// Synchronize directory contents.
#[derive(Debug)]
pub struct FSyncDir<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_fsync_in,
}
impl_request!(FSyncDir<'a>);
impl<'a> FSyncDir<'a> {
    /// The value set by the [OpenDir] method. See [FileHandle].
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    /// If set, then only the directory contents should be flushed, not the meta data.
    pub fn fdatasync(&self) -> bool {
        self.arg.fsync_flags & consts::FUSE_FSYNC_FDATASYNC != 0
    }
}

/// Test for a POSIX file lock.
#[derive(Debug)]
pub struct GetLk<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_lk_in,
}
impl_request!(GetLk<'a>);
impl<'a> GetLk<'a> {
    /// The value set by the [Open] method. See [FileHandle].
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn lock(&self) -> Lock {
        Lock::from_abi(&self.arg.lk)
    }
    pub fn lock_owner(&self) -> LockOwner {
        LockOwner(self.arg.owner)
    }
}

/// Acquire, modify or release a POSIX file lock.
///
/// For POSIX threads (NPTL) there's a 1-1 relation between pid and owner, but
/// otherwise this is not always the case.  For checking lock ownership,
/// 'fi->owner' must be used. The l_pid field in 'struct flock' should only be
/// used to fill in this field in getlk(). Note: if the locking methods are not
/// implemented, the kernel will still allow file locking to work locally.
/// Hence these are only interesting for network filesystems and similar.
#[derive(Debug)]
pub struct SetLk<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_lk_in,
}
impl_request!(SetLk<'a>);
impl<'a> SetLk<'a> {
    /// The value set by the [Open] method. See [FileHandle].
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn lock(&self) -> Lock {
        Lock::from_abi(&self.arg.lk)
    }
    pub fn lock_owner(&self) -> LockOwner {
        LockOwner(self.arg.owner)
    }
}
#[derive(Debug)]
pub struct SetLkW<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_lk_in,
}
impl_request!(SetLkW<'a>);
impl<'a> SetLkW<'a> {
    /// The value set by the [Open] method. See [FileHandle].
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn lock(&self) -> Lock {
        Lock::from_abi(&self.arg.lk)
    }
    pub fn lock_owner(&self) -> LockOwner {
        LockOwner(self.arg.owner)
    }
}

/// Check file access permissions.
///
/// This will be called for the `access()` system call. If the 'default_permissions'
/// mount option is given, this method is not called.
#[derive(Debug)]
pub struct Access<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_access_in,
}
impl_request!(Access<'a>);
impl<'a> Access<'a> {
    pub fn mask(&self) -> i32 {
        self.arg.mask
    }
}

/// Create and open a file.
///
/// If the file does not exist, first create it with the specified mode, and then
/// open it. Open flags (with the exception of `O_NOCTTY`) are available in flags.
/// Filesystem may store an arbitrary file handle (pointer, index, etc) in fh,
/// and use this in other all other file operations ([Read], [Write], [Flush], [Release],
/// [FSync]). There are also some flags (direct_io, keep_cache) which the
/// filesystem may set, to change the way the file is opened. See fuse_file_info
/// structure in <fuse_common.h> for more details. If this method is not
/// implemented or under Linux kernel versions earlier than 2.6.15, the [MkNod]
/// and [Open] methods will be called instead.
#[derive(Debug)]
pub struct Create<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_create_in,
    name: &'a Path,
}
impl_request!(Create<'a>);
impl<'a> Create<'a> {
    pub fn name(&self) -> &'a Path {
        self.name
    }
    pub fn mode(&self) -> u32 {
        self.arg.mode
    }
    /// Flags as passed to the creat() call
    pub fn flags(&self) -> i32 {
        self.arg.flags
    }
    pub fn umask(&self) -> u32 {
        #[cfg(not(feature = "abi-7-12"))]
        return 0;
        #[cfg(feature = "abi-7-12")]
        self.arg.umask
    }
}

/// If a process issuing a FUSE filesystem request is interrupted, the
/// following will happen:
///
///   1) If the request is not yet sent to userspace AND the signal is
///      fatal (SIGKILL or unhandled fatal signal), then the request is
///      dequeued and returns immediately.
///
///   2) If the request is not yet sent to userspace AND the signal is not
///      fatal, then an 'interrupted' flag is set for the request.  When
///      the request has been successfully transferred to userspace and
///      this flag is set, an INTERRUPT request is queued.
///
///   3) If the request is already sent to userspace, then an INTERRUPT
///      request is queued.
///
/// [Interrupt] requests take precedence over other requests, so the
/// userspace filesystem will receive queued [Interrupt]s before any others.
///
/// The userspace filesystem may ignore the [Interrupt] requests entirely,
/// or may honor them by sending a reply to the **original** request, with
/// the error set to [Errno::EINTR].
///
/// It is also possible that there's a race between processing the
/// original request and its [Interrupt] request.  There are two
/// possibilities:
///
/// 1. The [Interrupt] request is processed before the original request is
///    processed
///
/// 2. The [Interrupt] request is processed after the original request has
///    been answered
///
/// If the filesystem cannot find the original request, it should wait for
/// some timeout and/or a number of new requests to arrive, after which it
/// should reply to the [Interrupt] request with an [Errno::EAGAIN] error.
/// In case (1) the [Interrupt] request will be requeued.  In case (2) the
/// [Interrupt] reply will be ignored.
#[derive(Debug)]
pub struct Interrupt<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_interrupt_in,
}
impl_request!(Interrupt<'a>);
impl<'a> Interrupt<'a> {
    pub fn unique(&self) -> u64 {
        self.arg.unique
    }
}

/// Map block index within file to block index within device.
/// Note: This makes sense only for block device backed filesystems mounted
/// with the 'blkdev' option
#[derive(Debug)]
pub struct BMap<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_bmap_in,
}
impl_request!(BMap<'a>);
impl<'a> BMap<'a> {
    pub fn block_size(&self) -> u32 {
        self.arg.blocksize
    }
    pub fn block(&self) -> u64 {
        self.arg.block
    }
}

#[derive(Debug)]
pub struct Destroy<'a> {
    header: &'a fuse_in_header,
}
impl_request!(Destroy<'a>);
impl<'a> Destroy<'a> {
    pub fn reply(&self) -> Response<'a> {
        Response::new_empty()
    }
}

/// Control device
#[cfg(feature = "abi-7-11")]
#[derive(Debug)]
pub struct IoCtl<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_ioctl_in,
    data: &'a [u8],
}
#[cfg(feature = "abi-7-11")]
impl_request!(IoCtl<'a>);
#[cfg(feature = "abi-7-11")]
impl<'a> IoCtl<'a> {
    pub fn in_data(&self) -> &[u8] {
        &self.data[..self.arg.in_size as usize]
    }
    pub fn unrestricted(&self) -> bool {
        self.arg.flags & consts::FUSE_IOCTL_UNRESTRICTED != 0
    }
    /// The value set by the [Open] method. See [FileHandle].
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    /// TODO: What are valid values here?
    pub fn flags(&self) -> u32 {
        self.arg.flags
    }
    /// TODO: What does this mean?
    pub fn command(&self) -> u32 {
        self.arg.cmd
    }
    pub fn out_size(&self) -> u32 {
        self.arg.out_size
    }
}

/// Poll.  TODO: currently unsupported by fuser
#[cfg(feature = "abi-7-11")]
#[derive(Debug)]
pub struct Poll<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_poll_in,
}
#[cfg(feature = "abi-7-11")]
impl_request!(Poll<'a>);
#[cfg(feature = "abi-7-11")]
impl<'a> Poll<'a> {
    /// The value set by the [Open] method. See [FileHandle].
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
}

/// NotifyReply.  TODO: currently unsupported by fuser
#[cfg(feature = "abi-7-15")]
#[derive(Debug)]
pub struct NotifyReply<'a> {
    header: &'a fuse_in_header,
    #[allow(unused)]
    arg: &'a [u8],
}
#[cfg(feature = "abi-7-15")]
impl_request!(NotifyReply<'a>);

/// BatchForget: TODO: merge with Forget
#[cfg(feature = "abi-7-16")]
#[derive(Debug)]
pub struct BatchForget<'a> {
    header: &'a fuse_in_header,
    #[allow(unused)]
    arg: &'a fuse_batch_forget_in,
    nodes: &'a [fuse_forget_one],
}
#[cfg(feature = "abi-7-16")]
impl_request!(BatchForget<'a>);
#[cfg(feature = "abi-7-16")]
impl<'a> BatchForget<'a> {
    /// TODO: Don't return fuse_forget_one, this should be private
    pub fn nodes(&self) -> &'a [fuse_forget_one] {
        self.nodes
    }
}

/// Preallocate or deallocate space to a file
///
/// Implementations should return EINVAL if offset or length are < 0
#[cfg(feature = "abi-7-19")]
#[derive(Debug)]
pub struct FAllocate<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_fallocate_in,
}
#[cfg(feature = "abi-7-19")]
impl_request!(FAllocate<'a>);
#[cfg(feature = "abi-7-19")]
impl<'a> FAllocate<'a> {
    /// The value set by the [Open] method. See [FileHandle].
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn offset(&self) -> i64 {
        self.arg.offset
    }
    pub fn len(&self) -> i64 {
        self.arg.length
    }
    /// `mode` as passed to fallocate.  See `man 2 fallocate`
    pub fn mode(&self) -> i32 {
        self.arg.mode
    }
}

/// Read directory.
///
/// TODO: Document when this is called rather than ReadDirectory
#[cfg(feature = "abi-7-21")]
#[derive(Debug)]
pub struct ReadDirPlus<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_read_in,
}
#[cfg(feature = "abi-7-21")]
impl_request!(ReadDirPlus<'a>);
#[cfg(feature = "abi-7-21")]
impl<'a> ReadDirPlus<'a> {
    /// The value set by the [Open] method. See [FileHandle].
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn offset(&self) -> i64 {
        self.arg.offset
    }
    pub fn size(&self) -> u32 {
        self.arg.size
    }
}

/// Rename a file.
///
/// TODO: Document the differences to [Rename] and [Exchange]
#[cfg(feature = "abi-7-23")]
#[derive(Debug)]
pub struct Rename2<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_rename2_in,
    name: &'a Path,
    newname: &'a Path,
    old_parent: u64,
}
#[cfg(feature = "abi-7-23")]
impl_request!(Rename2<'a>);
#[cfg(feature = "abi-7-23")]
impl<'a> Rename2<'a> {
    pub fn from(&self) -> FilenameInDir<'a> {
        FilenameInDir::<'a> {
            dir: self.old_parent,
            name: self.name,
        }
    }
    pub fn to(&self) -> FilenameInDir<'a> {
        FilenameInDir::<'a> {
            dir: self.arg.newdir,
            name: self.newname,
        }
    }
    /// Flags as passed to renameat2.  As of Linux 3.18 this is
    /// [libc::RENAME_EXCHANGE], [libc::RENAME_NOREPLACE] and
    /// [libc::RENAME_WHITEOUT].  If you don't handle a particular flag
    /// reply with an EINVAL error.
    ///
    /// TODO: Replace with enum/flags type
    pub fn flags(&self) -> u32 {
        self.arg.flags
    }
}

/// Reposition read/write file offset
///
/// TODO: Document when you need to implement this.  Read and Write provide the offset anyway.
#[cfg(feature = "abi-7-24")]
#[derive(Debug)]
pub struct Lseek<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_lseek_in,
}
#[cfg(feature = "abi-7-24")]
impl_request!(Lseek<'a>);
#[cfg(feature = "abi-7-24")]
impl<'a> Lseek<'a> {
    /// The value set by the [Open] method. See [FileHandle].
    pub fn file_handle(&self) -> u64 {
        self.arg.fh
    }
    pub fn offset(&self) -> i64 {
        self.arg.offset
    }
    /// TODO: Make this return an enum
    pub fn whence(&self) -> i32 {
        self.arg.whence
    }
}

/// Copy the specified range from the source inode to the destination inode
#[derive(Debug, Clone, Copy)]
pub struct CopyFileRangeFile {
    pub inode: u64,
    /// The value set by the [Open] method. See [FileHandle].
    pub file_handle: u64,
    pub offset: i64,
}
#[cfg(feature = "abi-7-28")]
#[derive(Debug)]
pub struct CopyFileRange<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_copy_file_range_in,
}
#[cfg(feature = "abi-7-28")]
impl_request!(CopyFileRange<'a>);
#[cfg(feature = "abi-7-28")]
impl<'a> CopyFileRange<'a> {
    /// File and offset to copy data from
    pub fn src(&self) -> CopyFileRangeFile {
        CopyFileRangeFile {
            inode: self.header.nodeid,
            file_handle: self.arg.fh_in,
            offset: self.arg.off_in,
        }
    }
    /// File and offset to copy data to
    pub fn dest(&self) -> CopyFileRangeFile {
        CopyFileRangeFile {
            inode: self.arg.nodeid_out,
            file_handle: self.arg.fh_out,
            offset: self.arg.off_out,
        }
    }
    /// Number of bytes to copy
    pub fn len(&self) -> u64 {
        self.arg.len
    }
    // API TODO: Return a specific flags type
    pub fn flags(&self) -> u64 {
        self.arg.flags
    }
}

/// MacOS only: Rename the volume. Set `fuse_init_out.flags` during init to
/// `FUSE_VOL_RENAME` to enable
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct SetVolName<'a> {
    header: &'a fuse_in_header,
    name: &'a OsStr,
}
#[cfg(target_os = "macos")]
impl_request!(SetVolName<'a>);
#[cfg(target_os = "macos")]
impl<'a> SetVolName<'a> {
    pub fn name(&self) -> &'a OsStr {
        self.name
    }
}

/// macOS only: Query extended times (bkuptime and crtime). Set fuse_init_out.flags
/// during init to FUSE_XTIMES to enable
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct GetXTimes<'a> {
    header: &'a fuse_in_header,
}
#[cfg(target_os = "macos")]
impl_request!(GetXTimes<'a>);
// API TODO: Consider rename2(RENAME_EXCHANGE)
/// macOS only (undocumented)
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct Exchange<'a> {
    header: &'a fuse_in_header,
    arg: &'a fuse_exchange_in,
    oldname: &'a Path,
    newname: &'a Path,
}
#[cfg(target_os = "macos")]
impl_request!(Exchange<'a>);
#[cfg(target_os = "macos")]
impl<'a> Exchange<'a> {
    pub fn from(&self) -> FilenameInDir<'a> {
        FilenameInDir::<'a> {
            dir: self.arg.olddir,
            name: self.oldname,
        }
    }
    pub fn to(&self) -> FilenameInDir<'a> {
        FilenameInDir::<'a> {
            dir: self.arg.newdir,
            name: self.newname,
        }
    }
    pub fn options(&self) -> u64 {
        self.arg.options
    }
}
/// TODO: Document
#[cfg(feature = "abi-7-12")]
#[derive(Debug)]
pub struct CuseInit<'a> {
    header: &'a fuse_in_header,
    #[allow(unused)]
    arg: &'a fuse_init_in,
}
#[cfg(feature = "abi-7-12")]
impl_request!(CuseInit<'a>);

fn system_time_from_time(secs: i64, nsecs: u32) -> SystemTime {
    if secs >= 0 {
        SystemTime::UNIX_EPOCH + Duration::new(secs as u64, nsecs)
    } else {
        SystemTime::UNIX_EPOCH - Duration::new((-secs) as u64, nsecs)
    }
}
pub(crate) fn parse<'a>(
    header: &'a fuse_in_header,
    opcode: &fuse_opcode,
    data: &'a [u8],
) -> Option<Operation<'a>> {
    let mut data = ArgumentIterator::new(data);
    Some(match opcode {
        fuse_opcode::FUSE_LOOKUP => Operation::Lookup(Lookup {
            header,
            name: data.fetch_str()?,
        }),
        fuse_opcode::FUSE_FORGET => Operation::Forget(Forget {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_GETATTR => Operation::GetAttr(GetAttr { header }),
        fuse_opcode::FUSE_SETATTR => Operation::SetAttr(SetAttr {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_READLINK => Operation::ReadLink(ReadLink { header }),
        fuse_opcode::FUSE_SYMLINK => Operation::SymLink(SymLink {
            header,
            target: data.fetch_str()?.as_ref(),
            link: data.fetch_str()?.as_ref(),
        }),
        fuse_opcode::FUSE_MKNOD => Operation::MkNod(MkNod {
            header,
            arg: data.fetch()?,
            name: data.fetch_str()?.as_ref(),
        }),
        fuse_opcode::FUSE_MKDIR => Operation::MkDir(MkDir {
            header,
            arg: data.fetch()?,
            name: data.fetch_str()?.as_ref(),
        }),
        fuse_opcode::FUSE_UNLINK => Operation::Unlink(Unlink {
            header,
            name: data.fetch_str()?.as_ref(),
        }),
        fuse_opcode::FUSE_RMDIR => Operation::RmDir(RmDir {
            header,
            name: data.fetch_str()?.as_ref(),
        }),
        fuse_opcode::FUSE_RENAME => Operation::Rename(Rename {
            header,
            arg: data.fetch()?,
            name: data.fetch_str()?.as_ref(),
            newname: data.fetch_str()?.as_ref(),
        }),
        fuse_opcode::FUSE_LINK => Operation::Link(Link {
            header,
            arg: data.fetch()?,
            name: data.fetch_str()?.as_ref(),
        }),
        fuse_opcode::FUSE_OPEN => Operation::Open(Open {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_READ => Operation::Read(Read {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_WRITE => Operation::Write({
            let out = Write {
                header,
                arg: data.fetch()?,
                data: data.fetch_all(),
            };
            assert!(out.data().len() == out.arg.size as usize);
            out
        }),
        fuse_opcode::FUSE_STATFS => Operation::StatFs(StatFs { header }),
        fuse_opcode::FUSE_RELEASE => Operation::Release(Release {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_FSYNC => Operation::FSync(FSync {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_SETXATTR => Operation::SetXAttr({
            let out = SetXAttr {
                header,
                arg: data.fetch()?,
                name: data.fetch_str()?,
                value: data.fetch_all(),
            };
            assert!(out.value.len() == out.arg.size as usize);
            out
        }),
        fuse_opcode::FUSE_GETXATTR => Operation::GetXAttr(GetXAttr {
            header,
            arg: data.fetch()?,
            name: data.fetch_str()?,
        }),
        fuse_opcode::FUSE_LISTXATTR => Operation::ListXAttr(ListXAttr {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_REMOVEXATTR => Operation::RemoveXAttr(RemoveXAttr {
            header,
            name: data.fetch_str()?,
        }),
        fuse_opcode::FUSE_FLUSH => Operation::Flush(Flush {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_INIT => Operation::Init(Init {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_OPENDIR => Operation::OpenDir(OpenDir {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_READDIR => Operation::ReadDir(ReadDir {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_RELEASEDIR => Operation::ReleaseDir(ReleaseDir {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_FSYNCDIR => Operation::FSyncDir(FSyncDir {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_GETLK => Operation::GetLk(GetLk {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_SETLK => Operation::SetLk(SetLk {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_SETLKW => Operation::SetLkW(SetLkW {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_ACCESS => Operation::Access(Access {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_CREATE => Operation::Create(Create {
            header,
            arg: data.fetch()?,
            name: data.fetch_str()?.as_ref(),
        }),
        fuse_opcode::FUSE_INTERRUPT => Operation::Interrupt(Interrupt {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_BMAP => Operation::BMap(BMap {
            header,
            arg: data.fetch()?,
        }),
        fuse_opcode::FUSE_DESTROY => Operation::Destroy(Destroy { header }),
        #[cfg(feature = "abi-7-11")]
        fuse_opcode::FUSE_IOCTL => Operation::IoCtl(IoCtl {
            header,
            arg: data.fetch()?,
            data: data.fetch_all(),
        }),
        #[cfg(feature = "abi-7-11")]
        fuse_opcode::FUSE_POLL => Operation::Poll(Poll {
            header,
            arg: data.fetch()?,
        }),
        #[cfg(feature = "abi-7-15")]
        fuse_opcode::FUSE_NOTIFY_REPLY => Operation::NotifyReply(NotifyReply {
            header,
            arg: data.fetch_all(),
        }),
        #[cfg(feature = "abi-7-16")]
        fuse_opcode::FUSE_BATCH_FORGET => {
            let arg = data.fetch()?;
            Operation::BatchForget(BatchForget {
                header,
                arg,
                nodes: data.fetch_slice(arg.count as usize)?,
            })
        }
        #[cfg(feature = "abi-7-19")]
        fuse_opcode::FUSE_FALLOCATE => Operation::FAllocate(FAllocate {
            header,
            arg: data.fetch()?,
        }),
        #[cfg(feature = "abi-7-21")]
        fuse_opcode::FUSE_READDIRPLUS => Operation::ReadDirPlus(ReadDirPlus {
            header,
            arg: data.fetch()?,
        }),
        #[cfg(feature = "abi-7-23")]
        fuse_opcode::FUSE_RENAME2 => Operation::Rename2(Rename2 {
            header,
            arg: data.fetch()?,
            name: data.fetch_str()?.as_ref(),
            newname: data.fetch_str()?.as_ref(),
            old_parent: header.nodeid,
        }),
        #[cfg(feature = "abi-7-24")]
        fuse_opcode::FUSE_LSEEK => Operation::Lseek(Lseek {
            header,
            arg: data.fetch()?,
        }),
        #[cfg(feature = "abi-7-28")]
        fuse_opcode::FUSE_COPY_FILE_RANGE => Operation::CopyFileRange(CopyFileRange {
            header,
            arg: data.fetch()?,
        }),

        #[cfg(target_os = "macos")]
        fuse_opcode::FUSE_SETVOLNAME => Operation::SetVolName(SetVolName {
            header,
            name: data.fetch_str()?,
        }),
        #[cfg(target_os = "macos")]
        fuse_opcode::FUSE_GETXTIMES => Operation::GetXTimes(GetXTimes { header }),
        #[cfg(target_os = "macos")]
        fuse_opcode::FUSE_EXCHANGE => Operation::Exchange(Exchange {
            header,
            arg: data.fetch()?,
            oldname: data.fetch_str()?.as_ref(),
            newname: data.fetch_str()?.as_ref(),
        }),

        #[cfg(feature = "abi-7-12")]
        fuse_opcode::CUSE_INIT => Operation::CuseInit(CuseInit {
            header,
            arg: data.fetch()?,
        }),
    })
}
