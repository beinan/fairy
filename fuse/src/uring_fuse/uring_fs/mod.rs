use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use libc::ENOENT;
use log::debug;
use crate::uring_fuse::file_meta::FileType;

use crate::uring_fuse::filesystem::Filesystem;
use crate::uring_fuse::reply::reply_attr::ReplyAttr;
use crate::uring_fuse::reply::reply_entry::ReplyEntry;
use crate::uring_fuse::reply::reply_ops::{ReplyCreate, ReplyDirectory};
use crate::uring_fuse::request::Request;
use crate::uring_fuse::TimeOrNow;
use crate::uring_fuse::uring_fs::inode:: InodeManager;
use crate::uring_fuse::uring_fs::list_cache::ListStatusCache;

pub mod inode;
pub mod list_cache;

// const NUMFILES: u8 = 16;
// const MAXBYTES: u64 = 10;

pub struct UringFilesystem {
    inode_manager: InodeManager,
    ls_cache: ListStatusCache,
}

impl UringFilesystem {
    pub fn new(inode_manager: InodeManager, ls_cache: ListStatusCache) -> Self {
        Self {
            inode_manager,
            ls_cache,
        }
    }
}

impl Filesystem for UringFilesystem {
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        // Convert OsStr to String safely
        match name.to_os_string().into_string() {
            Ok(name_str) => {
                match self.inode_manager.lookup(parent, &name_str) {
                    Ok(inode) => reply.entry(
                        &Duration::from_millis(100000), &inode.into(), 0),
                    Err(_) => reply.error(ENOENT),
                }
            }
            Err(_) => {
                // Handle the case where `name` cannot be converted to a String.
                // ENOENT is used here to indicate that the entry is not present.
                reply.error(ENOENT);
            }
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match self.inode_manager.get(ino) {
            Ok(inode) => reply.attr(&Duration::from_millis(100000), &inode.into()),
            Err(_) => reply.error(ENOENT)
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if let Ok(dir_inode) = self.inode_manager.get(ino) {
            let mut entry_count = 0;
            if offset == 0 {
                let _ = reply.add(ino, 1,
                          FileType::Directory, ".");
                entry_count += 1;
            }

            if let Some(entries) = self.ls_cache.get(dir_inode.full_path()) {
                let entries = entries.iter().enumerate().skip(offset as usize);
                for (i, entry) in entries {
                    match self.inode_manager.lookup(ino, entry) {
                        Ok(entry_inode) => {
                            // Cast i to i64 to match the offset type.
                            let _ = reply.add(entry_inode.ino(), (i as i64) + offset + 2,
                                      entry_inode.kind(), &entry_inode.name());
                            entry_count += 1;
                        }
                        Err(_) => {
                            // Handle the error, e.g., by logging or breaking the loop.
                            // For now, we just continue to the next entry.
                            continue;
                        }
                    }
                }
            } else {
                // TODO: reload dir if ls_cache does not have the entry.

                //reply.error(ENOENT)
            }
            if entry_count > 0 {
                reply.ok()
            } else {
                reply.error(ENOENT)
            }
        } else {
            // Handle the error if inode_manager does not have the inode.
            reply.error(ENOENT)
        }

    }

    fn create(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        debug!(
            "create: {:?}/{:?} (mode={:#o}, flags={:#x})",
            parent, name, mode, flags
        );
        match self.inode_manager.get(parent) {
            Ok(parent_node) => {
                let full_path = PathBuf::from(parent_node.full_path()).join(name)
                    .into_os_string().into_string().unwrap();
                match self.inode_manager.create(parent,
                                                name.to_os_string().into_string().unwrap(),
                                                full_path,
                                                FileType::RegularFile) {
                    Ok(inode) => {
                        let ino = inode.ino();
                        self.ls_cache.append(String::from(parent_node.full_path()), name.to_os_string().into_string().unwrap());
                        reply.created(&Duration::from_millis(1000000), &inode.into(), ino, 5, 0)
                    }
                    Err(_) => reply.error(ENOENT)
                }
            }
            Err(_) => reply.error(ENOENT)
        }
    }

    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        debug!(
            "setattr(ino: {:#x?}, mode: {:?}, uid: {:?}, \
            gid: {:?}, size: {:?}, fh: {:?}, flags: {:?})",
            ino, mode, uid, gid, size, fh, flags
        );
        reply.attr(&Duration::from_millis(1000000), &self.inode_manager.get(ino).unwrap().into());
    }
}