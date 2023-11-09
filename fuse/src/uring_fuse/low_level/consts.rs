// Bitmasks for fuse_setattr_in.valid
pub const FATTR_MODE: u32 = 1 << 0;
pub const FATTR_UID: u32 = 1 << 1;
pub const FATTR_GID: u32 = 1 << 2;
pub const FATTR_SIZE: u32 = 1 << 3;
pub const FATTR_ATIME: u32 = 1 << 4;
pub const FATTR_MTIME: u32 = 1 << 5;
pub const FATTR_FH: u32 = 1 << 6;
#[cfg(feature = "abi-7-9")]
pub const FATTR_ATIME_NOW: u32 = 1 << 7;
#[cfg(feature = "abi-7-9")]
pub const FATTR_MTIME_NOW: u32 = 1 << 8;
#[cfg(feature = "abi-7-9")]
pub const FATTR_LOCKOWNER: u32 = 1 << 9;
#[cfg(feature = "abi-7-23")]
pub const FATTR_CTIME: u32 = 1 << 10;

#[cfg(target_os = "macos")]
pub const FATTR_CRTIME: u32 = 1 << 28;
#[cfg(target_os = "macos")]
pub const FATTR_CHGTIME: u32 = 1 << 29;
#[cfg(target_os = "macos")]
pub const FATTR_BKUPTIME: u32 = 1 << 30;
#[cfg(target_os = "macos")]
pub const FATTR_FLAGS: u32 = 1 << 31;

// Flags returned by the open request
pub const FOPEN_DIRECT_IO: u32 = 1 << 0; // bypass page cache for this open file
#[allow(dead_code)]
pub const FOPEN_KEEP_CACHE: u32 = 1 << 1; // don't invalidate the data cache on open
#[cfg(feature = "abi-7-10")]
pub const FOPEN_NONSEEKABLE: u32 = 1 << 2; // the file is not seekable
#[cfg(feature = "abi-7-28")]
pub const FOPEN_CACHE_DIR: u32 = 1 << 3; // allow caching this directory
#[cfg(feature = "abi-7-31")]
pub const FOPEN_STREAM: u32 = 1 << 4; // the file is stream-like (no file position at all)

#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub const FOPEN_PURGE_ATTR: u32 = 1 << 30;
#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub const FOPEN_PURGE_UBC: u32 = 1 << 31;

// Init request/reply flags
pub const FUSE_ASYNC_READ: u32 = 1 << 0; // asynchronous read requests
#[allow(dead_code)]
pub const FUSE_POSIX_LOCKS: u32 = 1 << 1; // remote locking for POSIX file locks
#[cfg(feature = "abi-7-9")]
pub const FUSE_FILE_OPS: u32 = 1 << 2; // kernel sends file handle for fstat, etc...
#[cfg(feature = "abi-7-9")]
pub const FUSE_ATOMIC_O_TRUNC: u32 = 1 << 3; // handles the O_TRUNC open flag in the filesystem
#[cfg(feature = "abi-7-10")]
pub const FUSE_EXPORT_SUPPORT: u32 = 1 << 4; // filesystem handles lookups of "." and ".."
#[cfg(feature = "abi-7-9")]
pub const FUSE_BIG_WRITES: u32 = 1 << 5; // filesystem can handle write size larger than 4kB
#[cfg(feature = "abi-7-12")]
pub const FUSE_DONT_MASK: u32 = 1 << 6; // don't apply umask to file mode on create operations
#[cfg(all(feature = "abi-7-14", not(target_os = "macos")))]
pub const FUSE_SPLICE_WRITE: u32 = 1 << 7; // kernel supports splice write on the device
#[cfg(all(feature = "abi-7-14", not(target_os = "macos")))]
pub const FUSE_SPLICE_MOVE: u32 = 1 << 8; // kernel supports splice move on the device
#[cfg(not(target_os = "macos"))]
#[cfg(feature = "abi-7-14")]
pub const FUSE_SPLICE_READ: u32 = 1 << 9; // kernel supports splice read on the device
#[cfg(feature = "abi-7-17")]
pub const FUSE_FLOCK_LOCKS: u32 = 1 << 10; // remote locking for BSD style file locks
#[cfg(feature = "abi-7-18")]
pub const FUSE_HAS_IOCTL_DIR: u32 = 1 << 11; // kernel supports ioctl on directories
#[cfg(feature = "abi-7-20")]
pub const FUSE_AUTO_INVAL_DATA: u32 = 1 << 12; // automatically invalidate cached pages
#[cfg(feature = "abi-7-21")]
pub const FUSE_DO_READDIRPLUS: u32 = 1 << 13; // do READDIRPLUS (READDIR+LOOKUP in one)
#[cfg(feature = "abi-7-21")]
pub const FUSE_READDIRPLUS_AUTO: u32 = 1 << 14; // adaptive readdirplus
#[cfg(feature = "abi-7-22")]
pub const FUSE_ASYNC_DIO: u32 = 1 << 15; // asynchronous direct I/O submission
#[cfg(feature = "abi-7-23")]
pub const FUSE_WRITEBACK_CACHE: u32 = 1 << 16; // use writeback cache for buffered writes
#[cfg(feature = "abi-7-23")]
pub const FUSE_NO_OPEN_SUPPORT: u32 = 1 << 17; // kernel supports zero-message opens
#[cfg(feature = "abi-7-25")]
pub const FUSE_PARALLEL_DIROPS: u32 = 1 << 18; // allow parallel lookups and readdir
#[cfg(feature = "abi-7-26")]
pub const FUSE_HANDLE_KILLPRIV: u32 = 1 << 19; // fs handles killing suid/sgid/cap on write/chown/trunc
#[cfg(feature = "abi-7-26")]
pub const FUSE_POSIX_ACL: u32 = 1 << 20; // filesystem supports posix acls
#[cfg(feature = "abi-7-27")]
pub const FUSE_ABORT_ERROR: u32 = 1 << 21; // reading the device after abort returns ECONNABORTED
#[cfg(feature = "abi-7-28")]
pub const FUSE_MAX_PAGES: u32 = 1 << 22; // init_out.max_pages contains the max number of req pages
#[cfg(feature = "abi-7-28")]
pub const FUSE_CACHE_SYMLINKS: u32 = 1 << 23; // cache READLINK responses
#[cfg(feature = "abi-7-29")]
pub const FUSE_NO_OPENDIR_SUPPORT: u32 = 1 << 24; // kernel supports zero-message opendir
#[cfg(feature = "abi-7-30")]
pub const FUSE_EXPLICIT_INVAL_DATA: u32 = 1 << 25; // only invalidate cached pages on explicit request

#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub const FUSE_ALLOCATE: u32 = 1 << 27;
#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub const FUSE_EXCHANGE_DATA: u32 = 1 << 28;
#[cfg(target_os = "macos")]
pub const FUSE_CASE_INSENSITIVE: u32 = 1 << 29;
#[cfg(target_os = "macos")]
pub const FUSE_VOL_RENAME: u32 = 1 << 30;
#[cfg(target_os = "macos")]
pub const FUSE_XTIMES: u32 = 1 << 31;

// CUSE init request/reply flags
#[cfg(feature = "abi-7-12")]
pub const CUSE_UNRESTRICTED_IOCTL: u32 = 1 << 0; // use unrestricted ioctl

// Release flags
pub const FUSE_RELEASE_FLUSH: u32 = 1 << 0;
#[cfg(feature = "abi-7-17")]
pub const FUSE_RELEASE_FLOCK_UNLOCK: u32 = 1 << 1;

// Getattr flags
#[cfg(feature = "abi-7-9")]
pub const FUSE_GETATTR_FH: u32 = 1 << 0;

// Lock flags
#[cfg(feature = "abi-7-9")]
pub const FUSE_LK_FLOCK: u32 = 1 << 0;

// Write flags
#[cfg(feature = "abi-7-9")]
pub const FUSE_WRITE_CACHE: u32 = 1 << 0; // delayed write from page cache, file handle is guessed
#[cfg(feature = "abi-7-9")]
pub const FUSE_WRITE_LOCKOWNER: u32 = 1 << 1; // lock_owner field is valid
#[cfg(feature = "abi-7-31")]
pub const FUSE_WRITE_KILL_PRIV: u32 = 1 << 2; // kill suid and sgid bits

// Read flags
#[cfg(feature = "abi-7-9")]
pub const FUSE_READ_LOCKOWNER: u32 = 1 << 1;

// IOCTL flags
#[cfg(feature = "abi-7-11")]
pub const FUSE_IOCTL_COMPAT: u32 = 1 << 0; // 32bit compat ioctl on 64bit machine
#[cfg(feature = "abi-7-11")]
pub const FUSE_IOCTL_UNRESTRICTED: u32 = 1 << 1; // not restricted to well-formed ioctls, retry allowed
#[cfg(feature = "abi-7-11")]
pub const FUSE_IOCTL_RETRY: u32 = 1 << 2; // retry with new iovecs
#[cfg(feature = "abi-7-16")]
pub const FUSE_IOCTL_32BIT: u32 = 1 << 3; // 32bit ioctl
#[cfg(feature = "abi-7-18")]
pub const FUSE_IOCTL_DIR: u32 = 1 << 4; // is a directory
#[cfg(feature = "abi-7-30")]
pub const FUSE_IOCTL_COMPAT_X32: u32 = 1 << 5; // x32 compat ioctl on 64bit machine (64bit time_t)
#[cfg(feature = "abi-7-11")]
pub const FUSE_IOCTL_MAX_IOV: u32 = 256; // maximum of in_iovecs + out_iovecs

// Poll flags
#[cfg(feature = "abi-7-9")]
pub const FUSE_POLL_SCHEDULE_NOTIFY: u32 = 1 << 0; // request poll notify

// fsync flags
pub const FUSE_FSYNC_FDATASYNC: u32 = 1 << 0; // Sync data only, not metadata

// The read buffer is required to be at least 8k, but may be much larger
#[allow(dead_code)]
pub const FUSE_MIN_READ_BUFFER: usize = 8192;