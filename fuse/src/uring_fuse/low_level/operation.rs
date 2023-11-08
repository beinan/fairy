use std::fmt;

use super::op::*;

/// Filesystem operation (and arguments) the kernel driver wants us to perform. The fields of each
/// variant needs to match the actual arguments the kernel driver sends for the specific operation.
#[derive(Debug)]
#[allow(missing_docs)]
pub enum Operation<'a> {
    Lookup(Lookup<'a>),
    Forget(Forget<'a>),
    GetAttr(GetAttr<'a>),
    SetAttr(SetAttr<'a>),
    ReadLink(ReadLink<'a>),
    SymLink(SymLink<'a>),
    MkNod(MkNod<'a>),
    MkDir(MkDir<'a>),
    Unlink(Unlink<'a>),
    RmDir(RmDir<'a>),
    Rename(Rename<'a>),
    Link(Link<'a>),
    Open(Open<'a>),
    Read(Read<'a>),
    Write(Write<'a>),
    StatFs(StatFs<'a>),
    Release(Release<'a>),
    FSync(FSync<'a>),
    SetXAttr(SetXAttr<'a>),
    GetXAttr(GetXAttr<'a>),
    ListXAttr(ListXAttr<'a>),
    RemoveXAttr(RemoveXAttr<'a>),
    Flush(Flush<'a>),
    Init(Init<'a>),
    OpenDir(OpenDir<'a>),
    ReadDir(ReadDir<'a>),
    ReleaseDir(ReleaseDir<'a>),
    FSyncDir(FSyncDir<'a>),
    GetLk(GetLk<'a>),
    SetLk(SetLk<'a>),
    SetLkW(SetLkW<'a>),
    Access(Access<'a>),
    Create(Create<'a>),
    Interrupt(Interrupt<'a>),
    BMap(BMap<'a>),
    Destroy(Destroy<'a>),
    #[cfg(feature = "abi-7-11")]
    IoCtl(IoCtl<'a>),
    #[cfg(feature = "abi-7-11")]
    Poll(Poll<'a>),
    #[cfg(feature = "abi-7-15")]
    NotifyReply(NotifyReply<'a>),
    #[cfg(feature = "abi-7-16")]
    BatchForget(BatchForget<'a>),
    #[cfg(feature = "abi-7-19")]
    FAllocate(FAllocate<'a>),
    #[cfg(feature = "abi-7-21")]
    ReadDirPlus(ReadDirPlus<'a>),
    #[cfg(feature = "abi-7-23")]
    Rename2(Rename2<'a>),
    #[cfg(feature = "abi-7-24")]
    Lseek(Lseek<'a>),
    #[cfg(feature = "abi-7-28")]
    CopyFileRange(CopyFileRange<'a>),

    #[cfg(target_os = "macos")]
    SetVolName(SetVolName<'a>),
    #[cfg(target_os = "macos")]
    GetXTimes(GetXTimes<'a>),
    #[cfg(target_os = "macos")]
    Exchange(Exchange<'a>),

    #[cfg(feature = "abi-7-12")]
    CuseInit(CuseInit<'a>),
}

impl<'a> fmt::Display for Operation<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operation::Lookup(x) => write!(f, "LOOKUP name {:?}", x.name()),
            Operation::Forget(x) => write!(f, "FORGET nlookup {}", x.nlookup()),
            Operation::GetAttr(_) => write!(f, "GETATTR"),
            Operation::SetAttr(x) => x.fmt(f),
            Operation::ReadLink(_) => write!(f, "READLINK"),
            Operation::SymLink(x) => {
                write!(f, "SYMLINK target {:?}, link {:?}", x.target(), x.link())
            }
            Operation::MkNod(x) => write!(
                f,
                "MKNOD name {:?}, mode {:#05o}, rdev {}",
                x.name(),
                x.mode(),
                x.rdev()
            ),
            Operation::MkDir(x) => write!(f, "MKDIR name {:?}, mode {:#05o}", x.name(), x.mode()),
            Operation::Unlink(x) => write!(f, "UNLINK name {:?}", x.name()),
            Operation::RmDir(x) => write!(f, "RMDIR name {:?}", x.name),
            Operation::Rename(x) => write!(f, "RENAME src {:?}, dest {:?}", x.src(), x.dest()),
            Operation::Link(x) => write!(f, "LINK ino {:?}, dest {:?}", x.inode_no(), x.dest()),
            Operation::Open(x) => write!(f, "OPEN flags {:#x}", x.flags()),
            Operation::Read(x) => write!(
                f,
                "READ fh {:?}, offset {}, size {}",
                x.file_handle(),
                x.offset(),
                x.size()
            ),
            Operation::Write(x) => write!(
                f,
                "WRITE fh {:?}, offset {}, size {}, write flags {:#x}",
                x.file_handle(),
                x.offset(),
                x.data().len(),
                x.write_flags()
            ),
            Operation::StatFs(_) => write!(f, "STATFS"),
            Operation::Release(x) => write!(
                f,
                "RELEASE fh {:?}, flags {:#x}, flush {}, lock owner {:?}",
                x.file_handle(),
                x.flags(),
                x.flush(),
                x.lock_owner()
            ),
            Operation::FSync(x) => write!(
                f,
                "FSYNC fh {:?}, fsync fdatasync {}",
                x.file_handle(),
                x.fdatasync()
            ),
            Operation::SetXAttr(x) => write!(
                f,
                "SETXATTR name {:?}, size {}, flags {:#x}",
                x.name(),
                x.value().len(),
                x.flags()
            ),
            Operation::GetXAttr(x) => {
                write!(f, "GETXATTR name {:?}, size {:?}", x.name(), x.size())
            }
            Operation::ListXAttr(x) => write!(f, "LISTXATTR size {}", x.size()),
            Operation::RemoveXAttr(x) => write!(f, "REMOVEXATTR name {:?}", x.name()),
            Operation::Flush(x) => write!(
                f,
                "FLUSH fh {:?}, lock owner {:?}",
                x.file_handle(),
                x.lock_owner()
            ),
            Operation::Init(x) => write!(
                f,
                "INIT kernel ABI {}, capabilities {:#x}, max readahead {}",
                x.version(),
                x.capabilities(),
                x.max_readahead()
            ),
            Operation::OpenDir(x) => write!(f, "OPENDIR flags {:#x}", x.flags()),
            Operation::ReadDir(x) => write!(
                f,
                "READDIR fh {:?}, offset {}, size {}",
                x.file_handle(),
                x.offset(),
                x.size()
            ),
            Operation::ReleaseDir(x) => write!(
                f,
                "RELEASEDIR fh {:?}, flags {:#x}, flush {}, lock owner {:?}",
                x.file_handle(),
                x.flags(),
                x.flush(),
                x.lock_owner()
            ),
            Operation::FSyncDir(x) => write!(
                f,
                "FSYNCDIR fh {:?}, fsync fdatasync: {}",
                x.file_handle(),
                x.fdatasync()
            ),
            Operation::GetLk(x) => write!(
                f,
                "GETLK fh {:?}, lock owner {:?}",
                x.file_handle(),
                x.lock_owner()
            ),
            Operation::SetLk(x) => write!(
                f,
                "SETLK fh {:?}, lock owner {:?}",
                x.file_handle(),
                x.lock_owner()
            ),
            Operation::SetLkW(x) => write!(
                f,
                "SETLKW fh {:?}, lock owner {:?}",
                x.file_handle(),
                x.lock_owner()
            ),
            Operation::Access(x) => write!(f, "ACCESS mask {:#05o}", x.mask()),
            Operation::Create(x) => write!(
                f,
                "CREATE name {:?}, mode {:#05o}, flags {:#x}",
                x.name(),
                x.mode(),
                x.flags()
            ),
            Operation::Interrupt(x) => write!(f, "INTERRUPT unique {:?}", x.unique()),
            Operation::BMap(x) => write!(f, "BMAP blocksize {}, ids {}", x.block_size(), x.block()),
            Operation::Destroy(_) => write!(f, "DESTROY"),
            #[cfg(feature = "abi-7-11")]
            Operation::IoCtl(x) => write!(
                f,
                "IOCTL fh {:?}, cmd {}, data size {}, flags {:#x}",
                x.file_handle(),
                x.command(),
                x.in_data().len(),
                x.flags()
            ),
            #[cfg(feature = "abi-7-11")]
            Operation::Poll(x) => write!(f, "POLL fh {:?}", x.file_handle()),
            #[cfg(feature = "abi-7-15")]
            Operation::NotifyReply(_) => write!(f, "NOTIFYREPLY"),
            #[cfg(feature = "abi-7-16")]
            Operation::BatchForget(x) => write!(f, "BATCHFORGET nodes {:?}", x.nodes()),
            #[cfg(feature = "abi-7-19")]
            Operation::FAllocate(_) => write!(f, "FALLOCATE"),
            #[cfg(feature = "abi-7-21")]
            Operation::ReadDirPlus(x) => write!(
                f,
                "READDIRPLUS fh {:?}, offset {}, size {}",
                x.file_handle(),
                x.offset(),
                x.size()
            ),
            #[cfg(feature = "abi-7-23")]
            Operation::Rename2(x) => write!(f, "RENAME2 from {:?}, to {:?}", x.from(), x.to()),
            #[cfg(feature = "abi-7-24")]
            Operation::Lseek(x) => write!(
                f,
                "LSEEK fh {:?}, offset {}, whence {}",
                x.file_handle(),
                x.offset(),
                x.whence()
            ),
            #[cfg(feature = "abi-7-28")]
            Operation::CopyFileRange(x) => write!(
                f,
                "COPY_FILE_RANGE src {:?}, dest {:?}, len {}",
                x.src(),
                x.dest(),
                x.len()
            ),

            #[cfg(target_os = "macos")]
            Operation::SetVolName(x) => write!(f, "SETVOLNAME name {:?}", x.name()),
            #[cfg(target_os = "macos")]
            Operation::GetXTimes(_) => write!(f, "GETXTIMES"),
            #[cfg(target_os = "macos")]
            Operation::Exchange(x) => write!(
                f,
                "EXCHANGE from {:?}, to {:?}, options {:#x}",
                x.from(),
                x.to(),
                x.options()
            ),

            #[cfg(feature = "abi-7-12")]
            Operation::CuseInit(_) => write!(f, "CUSE_INIT"),
        }
    }
}
