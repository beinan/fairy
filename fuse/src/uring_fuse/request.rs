use std::path::Path;

use log::{debug, error, warn};

use crate::uring_fuse::reply::ReplySender;
use crate::uring_fuse::{
    low_level::{
        kernel_interface::{FUSE_KERNEL_MINOR_VERSION, FUSE_KERNEL_VERSION},
        operation::Operation,
        version::Version,
    },
    KernelConfig,
};

use super::low_level::request::Request as ll_request;
use super::{
    channel::ChannelSender,
    filesystem::Filesystem,
    low_level::{errno::Errno, op::AnyRequest, response::Response},
    reply::{
        reply_ops::{ReplyDirectory, ReplyDirectoryPlus},
        Reply,
    },
    session::{Session, SessionACL},
};

pub struct Request<'a> {
    /// Channel sender for sending the reply
    ch: ChannelSender,
    /// Request raw data
    #[allow(unused)]
    data: &'a [u8],
    /// Parsed request
    request: AnyRequest<'a>,
}

#[allow(dead_code)]
impl<'a> Request<'a> {
    /// Create a new request from the given data
    pub(crate) fn new(ch: ChannelSender, data: &'a [u8]) -> Option<Request<'a>> {
        let request = match AnyRequest::try_from(data) {
            Ok(request) => request,
            Err(err) => {
                error!("{}", err);
                return None;
            }
        };

        Some(Self { ch, data, request })
    }

    /// Dispatch request to the given filesystem.
    /// This calls the appropriate filesystem operation method for the
    /// request and sends back the returned reply to the kernel
    pub(crate) fn dispatch<FS: Filesystem>(&self, se: &mut Session<FS>) {
        debug!("dispatching {}", self.request);
        let unique = self.request.unique();

        let res = match self.dispatch_req(se) {
            Ok(Some(resp)) => resp,
            Ok(None) => return,
            Err(errno) => self.request.reply_err(errno),
        }
        .with_iovec(unique, |iov| self.ch.send(iov));

        if let Err(err) = res {
            warn!("Request {:?}: Failed to send reply: {}", unique, err)
        }
    }

    fn dispatch_req<FS: Filesystem>(
        &self,
        se: &mut Session<FS>,
    ) -> Result<Option<Response<'_>>, Errno> {
        let op = self.request.operation().map_err(|_| Errno::ENOSYS)?;
        // Implement allow_root & access check for auto_unmount
        if (se.allowed == SessionACL::RootAndOwner
            && self.request.uid() != se.session_owner
            && self.request.uid() != 0)
            || (se.allowed == SessionACL::Owner && self.request.uid() != se.session_owner)
        {
            #[cfg(feature = "abi-7-21")]
            {
                match op {
                    // Only allow operations that the kernel may issue without a uid set
                    Operation::Init(_)
                    | Operation::Destroy(_)
                    | Operation::Read(_)
                    | Operation::ReadDir(_)
                    | Operation::ReadDirPlus(_)
                    | Operation::BatchForget(_)
                    | Operation::Forget(_)
                    | Operation::Write(_)
                    | Operation::FSync(_)
                    | Operation::FSyncDir(_)
                    | Operation::Release(_)
                    | Operation::ReleaseDir(_) => {}
                    _ => {
                        return Err(Errno::EACCES);
                    }
                }
            }
            #[cfg(all(feature = "abi-7-16", not(feature = "abi-7-21")))]
            {
                match op {
                    // Only allow operations that the kernel may issue without a uid set
                    Operation::Init(_)
                    | Operation::Destroy(_)
                    | Operation::Read(_)
                    | Operation::ReadDir(_)
                    | Operation::BatchForget(_)
                    | Operation::Forget(_)
                    | Operation::Write(_)
                    | Operation::FSync(_)
                    | Operation::FSyncDir(_)
                    | Operation::Release(_)
                    | Operation::ReleaseDir(_) => {}
                    _ => {
                        return Err(Errno::EACCES);
                    }
                }
            }
            #[cfg(not(feature = "abi-7-16"))]
            {
                match op {
                    // Only allow operations that the kernel may issue without a uid set
                    Operation::Init(_)
                    | Operation::Destroy(_)
                    | Operation::Read(_)
                    | Operation::ReadDir(_)
                    | Operation::Forget(_)
                    | Operation::Write(_)
                    | Operation::FSync(_)
                    | Operation::FSyncDir(_)
                    | Operation::Release(_)
                    | Operation::ReleaseDir(_) => {}
                    _ => {
                        return Err(Errno::EACCES);
                    }
                }
            }
        }
        match op {
            // Filesystem initialization
            Operation::Init(x) => {
                // We don't support ABI versions before 7.6
                let v = x.version();
                if v < Version(7, 6) {
                    error!("Unsupported FUSE ABI version {}", v);
                    return Err(Errno::EPROTO);
                }
                // Remember ABI version supported by kernel
                se.proto_major = v.major();
                se.proto_minor = v.minor();

                let mut config = KernelConfig::new(x.capabilities(), x.max_readahead());
                // Call filesystem init method and give it a chance to return an error
                se.filesystem
                    .init(self, &mut config)
                    .map_err(Errno::from_i32)?;

                // Reply with our desired version and settings. If the kernel supports a
                // larger major version, it'll re-send a matching init message. If it
                // supports only lower major versions, we replied with an error above.
                debug!(
                    "INIT response: ABI {}.{}, flags {:#x}, max readahead {}, max write {}",
                    FUSE_KERNEL_VERSION,
                    FUSE_KERNEL_MINOR_VERSION,
                    x.capabilities() & config.requested,
                    config.max_readahead,
                    config.max_write
                );
                se.initialized = true;
                return Ok(Some(x.reply(&config)));
            }
            // Any operation is invalid before initialization
            _ if !se.initialized => {
                warn!("Ignoring FUSE operation before init: {}", self.request);
                return Err(Errno::EIO);
            }
            // Filesystem destroyed
            Operation::Destroy(x) => {
                se.filesystem.destroy();
                se.destroyed = true;
                return Ok(Some(x.reply()));
            }
            // Any operation is invalid after destroy
            _ if se.destroyed => {
                warn!("Ignoring FUSE operation after destroy: {}", self.request);
                return Err(Errno::EIO);
            }

            Operation::Interrupt(_) => {
                // TODO: handle FUSE_INTERRUPT
                return Err(Errno::ENOSYS);
            }

            Operation::Lookup(x) => {
                se.filesystem
                    .lookup(self, self.request.nodeid(), x.name().as_ref(), self.reply());
            }
            Operation::Forget(x) => {
                se.filesystem
                    .forget(self, self.request.nodeid(), x.nlookup()); // no reply
            }
            Operation::GetAttr(_) => {
                se.filesystem
                    .getattr(self, self.request.nodeid(), self.reply());
            }
            Operation::SetAttr(x) => {
                se.filesystem.setattr(
                    self,
                    self.request.nodeid(),
                    x.mode(),
                    x.uid(),
                    x.gid(),
                    x.size(),
                    x.atime(),
                    x.mtime(),
                    x.ctime(),
                    x.file_handle(),
                    x.crtime(),
                    x.chgtime(),
                    x.bkuptime(),
                    x.flags(),
                    self.reply(),
                );
            }
            Operation::ReadLink(_) => {
                se.filesystem
                    .readlink(self, self.request.nodeid(), self.reply());
            }
            Operation::MkNod(x) => {
                se.filesystem.mknod(
                    self,
                    self.request.nodeid(),
                    x.name().as_ref(),
                    x.mode(),
                    x.umask(),
                    x.rdev(),
                    self.reply(),
                );
            }
            Operation::MkDir(x) => {
                se.filesystem.mkdir(
                    self,
                    self.request.nodeid(),
                    x.name().as_ref(),
                    x.mode(),
                    x.umask(),
                    self.reply(),
                );
            }
            Operation::Unlink(x) => {
                se.filesystem
                    .unlink(self, self.request.nodeid(), x.name().as_ref(), self.reply());
            }
            Operation::RmDir(x) => {
                se.filesystem
                    .rmdir(self, self.request.nodeid(), x.name().as_ref(), self.reply());
            }
            Operation::SymLink(x) => {
                se.filesystem.symlink(
                    self,
                    self.request.nodeid(),
                    x.target().as_ref(),
                    Path::new(x.link()),
                    self.reply(),
                );
            }
            Operation::Rename(x) => {
                se.filesystem.rename(
                    self,
                    self.request.nodeid(),
                    x.src().name.as_ref(),
                    x.dest().dir,
                    x.dest().name.as_ref(),
                    0,
                    self.reply(),
                );
            }
            Operation::Link(x) => {
                se.filesystem.link(
                    self,
                    x.inode_no(),
                    self.request.nodeid(),
                    x.dest().name.as_ref(),
                    self.reply(),
                );
            }
            Operation::Open(x) => {
                se.filesystem
                    .open(self, self.request.nodeid(), x.flags(), self.reply());
            }
            Operation::Read(x) => {
                se.filesystem.read(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.offset(),
                    x.size(),
                    x.flags(),
                    x.lock_owner().map(|l| l.into()),
                    self.reply(),
                );
            }
            Operation::Write(x) => {
                se.filesystem.write(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.offset(),
                    x.data(),
                    x.write_flags(),
                    x.flags(),
                    x.lock_owner().map(|l| l.into()),
                    self.reply(),
                );
            }
            Operation::Flush(x) => {
                se.filesystem.flush(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.lock_owner().into(),
                    self.reply(),
                );
            }
            Operation::Release(x) => {
                se.filesystem.release(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.flags(),
                    x.lock_owner().map(|x| x.into()),
                    x.flush(),
                    self.reply(),
                );
            }
            Operation::FSync(x) => {
                se.filesystem.fsync(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.fdatasync(),
                    self.reply(),
                );
            }
            Operation::OpenDir(x) => {
                se.filesystem
                    .opendir(self, self.request.nodeid(), x.flags(), self.reply());
            }
            Operation::ReadDir(x) => {
                se.filesystem.readdir(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.offset(),
                    ReplyDirectory::new(self.request.unique(), self.ch.clone(), x.size() as usize),
                );
            }
            Operation::ReleaseDir(x) => {
                se.filesystem.releasedir(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.flags(),
                    self.reply(),
                );
            }
            Operation::FSyncDir(x) => {
                se.filesystem.fsyncdir(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.fdatasync(),
                    self.reply(),
                );
            }
            Operation::StatFs(_) => {
                se.filesystem
                    .statfs(self, self.request.nodeid(), self.reply());
            }
            Operation::SetXAttr(x) => {
                se.filesystem.setxattr(
                    self,
                    self.request.nodeid(),
                    x.name(),
                    x.value(),
                    x.flags(),
                    x.position(),
                    self.reply(),
                );
            }
            Operation::GetXAttr(x) => {
                se.filesystem.getxattr(
                    self,
                    self.request.nodeid(),
                    x.name(),
                    x.size_u32(),
                    self.reply(),
                );
            }
            Operation::ListXAttr(x) => {
                se.filesystem
                    .listxattr(self, self.request.nodeid(), x.size(), self.reply());
            }
            Operation::RemoveXAttr(x) => {
                se.filesystem
                    .removexattr(self, self.request.nodeid(), x.name(), self.reply());
            }
            Operation::Access(x) => {
                se.filesystem
                    .access(self, self.request.nodeid(), x.mask(), self.reply());
            }
            Operation::Create(x) => {
                se.filesystem.create(
                    self,
                    self.request.nodeid(),
                    x.name().as_ref(),
                    x.mode(),
                    x.umask(),
                    x.flags(),
                    self.reply(),
                );
            }
            Operation::GetLk(x) => {
                se.filesystem.getlk(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.lock_owner().into(),
                    x.lock().range.0,
                    x.lock().range.1,
                    x.lock().typ,
                    x.lock().pid,
                    self.reply(),
                );
            }
            Operation::SetLk(x) => {
                se.filesystem.setlk(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.lock_owner().into(),
                    x.lock().range.0,
                    x.lock().range.1,
                    x.lock().typ,
                    x.lock().pid,
                    false,
                    self.reply(),
                );
            }
            Operation::SetLkW(x) => {
                se.filesystem.setlk(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.lock_owner().into(),
                    x.lock().range.0,
                    x.lock().range.1,
                    x.lock().typ,
                    x.lock().pid,
                    true,
                    self.reply(),
                );
            }
            Operation::BMap(x) => {
                se.filesystem.bmap(
                    self,
                    self.request.nodeid(),
                    x.block_size(),
                    x.block(),
                    self.reply(),
                );
            }

            #[cfg(feature = "abi-7-11")]
            Operation::IoCtl(x) => {
                if x.unrestricted() {
                    return Err(Errno::ENOSYS);
                } else {
                    se.filesystem.ioctl(
                        self,
                        self.request.nodeid(),
                        x.file_handle(),
                        x.flags(),
                        x.command(),
                        x.in_data(),
                        x.out_size(),
                        self.reply(),
                    );
                }
            }
            #[cfg(feature = "abi-7-11")]
            Operation::Poll(_) => {
                // TODO: handle FUSE_POLL
                return Err(Errno::ENOSYS);
            }
            #[cfg(feature = "abi-7-15")]
            Operation::NotifyReply(_) => {
                // TODO: handle FUSE_NOTIFY_REPLY
                return Err(Errno::ENOSYS);
            }
            #[cfg(feature = "abi-7-16")]
            Operation::BatchForget(x) => {
                se.filesystem.batch_forget(self, x.nodes()); // no reply
            }
            #[cfg(feature = "abi-7-19")]
            Operation::FAllocate(x) => {
                se.filesystem.fallocate(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.offset(),
                    x.len(),
                    x.mode(),
                    self.reply(),
                );
            }
            #[cfg(feature = "abi-7-21")]
            Operation::ReadDirPlus(x) => {
                se.filesystem.readdirplus(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.offset(),
                    ReplyDirectoryPlus::new(
                        self.request.unique(),
                        self.ch.clone(),
                        x.size() as usize,
                    ),
                );
            }
            #[cfg(feature = "abi-7-23")]
            Operation::Rename2(x) => {
                se.filesystem.rename(
                    self,
                    x.from().dir,
                    x.from().name.as_ref(),
                    x.to().dir,
                    x.to().name.as_ref(),
                    x.flags(),
                    self.reply(),
                );
            }
            #[cfg(feature = "abi-7-24")]
            Operation::Lseek(x) => {
                se.filesystem.lseek(
                    self,
                    self.request.nodeid(),
                    x.file_handle(),
                    x.offset(),
                    x.whence(),
                    self.reply(),
                );
            }
            #[cfg(feature = "abi-7-28")]
            Operation::CopyFileRange(x) => {
                let (i, o) = (x.src(), x.dest());
                se.filesystem.copy_file_range(
                    self,
                    i.inode,
                    i.file_handle,
                    i.offset,
                    o.inode,
                    o.file_handle,
                    o.offset,
                    x.len(),
                    x.flags().try_into().unwrap(),
                    self.reply(),
                );
            }
            #[cfg(target_os = "macos")]
            Operation::SetVolName(x) => {
                se.filesystem.setvolname(self, x.name(), self.reply());
            }
            #[cfg(target_os = "macos")]
            Operation::GetXTimes(_) => {
                se.filesystem
                    .getxtimes(self, self.request.nodeid(), self.reply());
            }
            #[cfg(target_os = "macos")]
            Operation::Exchange(x) => {
                se.filesystem.exchange(
                    self,
                    x.from().dir,
                    x.from().name.as_ref(),
                    x.to().dir,
                    x.to().name.as_ref(),
                    x.options(),
                    self.reply(),
                );
            }

            #[cfg(feature = "abi-7-12")]
            Operation::CuseInit(_) => {
                // TODO: handle CUSE_INIT
                return Err(Errno::ENOSYS);
            }
        }
        Ok(None)
    }

    /// Create a reply object for this request that can be passed to the filesystem
    /// implementation and makes sure that a request is replied exactly once
    fn reply<T: Reply>(&self) -> T {
        Reply::new(self.request.unique(), self.ch.clone())
    }

    /// Returns the unique identifier of this request
    #[inline]
    pub fn unique(&self) -> u64 {
        self.request.unique()
    }

    /// Returns the uid of this request
    #[inline]
    pub fn uid(&self) -> u32 {
        self.request.uid()
    }

    /// Returns the gid of this request
    #[inline]
    pub fn gid(&self) -> u32 {
        self.request.gid()
    }

    /// Returns the pid of this request
    #[inline]
    pub fn pid(&self) -> u32 {
        self.request.pid()
    }
}
