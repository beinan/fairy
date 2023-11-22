use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::UNIX_EPOCH;

use thiserror::Error;

use crate::uring_fuse::file_meta::{FileAttr, FileType};
use crate::uring_fuse::low_level::kernel_interface::FUSE_ROOT_ID;

pub type InodeNo = u64;
pub struct InodeManager {
    inner: Arc<InodeManagerInner>,
}

impl InodeManager {
    pub fn new(bucket: &str) -> Self {
        let root = Inode::new(
            FUSE_ROOT_ID,
            FUSE_ROOT_ID,
            String::new(),
            String::from("/"),
            FileType::Directory,
        );

        let mut inodes = HashMap::new();
        inodes.insert(FUSE_ROOT_ID, root);

        let mut path_indexes = HashMap::new();
        path_indexes.insert(String::from("/"), FUSE_ROOT_ID);

        let inner = InodeManagerInner {
            bucket: bucket.to_owned(),
            inodes: RwLock::new(inodes),
            path_index: RwLock::new(path_indexes),
            next_ino: AtomicU64::new(2),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn get(&self, ino: u64) -> Result<Inode, InodeError> {
        self.inner.get(ino)
    }

    pub fn lookup(&self, parent_ino: u64, name: &str) -> Result<Inode, InodeError> {
        self.inner.lookup(parent_ino, name)
    }

    pub fn create(
        &self,
        parent: u64,
        name: String,
        full_path: String,
        kind: FileType,
    ) -> Result<Inode, InodeError> {
        self.inner.create(parent, name, full_path, kind)
    }
}

#[allow(dead_code)]
struct InodeManagerInner {
    bucket: String,
    inodes: RwLock<HashMap<InodeNo, Inode>>,
    path_index: RwLock<HashMap<String, InodeNo>>,
    next_ino: AtomicU64,
}

impl InodeManagerInner {
    pub fn get(&self, ino: InodeNo) -> Result<Inode, InodeError> {
        let inode = self
            .inodes
            .read()
            .unwrap()
            .get(&ino)
            .cloned()
            .ok_or(InodeError::InodeDoesNotExist(ino))?;
        Ok(inode)
    }

    pub fn lookup(&self, parent_ino: InodeNo, name: &str) -> Result<Inode, InodeError> {
        let parent_inode = self
            .inodes
            .read()
            .unwrap()
            .get(&parent_ino)
            .cloned()
            .ok_or(InodeError::InodeDoesNotExist(parent_ino))?;
        let full_path = PathBuf::from(parent_inode.full_path())
            .join(name)
            .into_os_string()
            .into_string()
            .unwrap();
        let ino = *self
            .path_index
            .read()
            .unwrap()
            .get(full_path.as_str())
            .ok_or(InodeError::InodeDoesNotExist(parent_ino))?;
        let inode = self
            .inodes
            .read()
            .unwrap()
            .get(&ino)
            .cloned()
            .ok_or(InodeError::InodeDoesNotExist(ino))?;
        Ok(inode)
    }

    pub fn create(
        &self,
        parent: InodeNo,
        name: String,
        full_path: String,
        kind: FileType,
    ) -> Result<Inode, InodeError> {
        let inode_id_new = self.next_ino.fetch_add(1, Ordering::SeqCst);
        let inode_new = Inode::new(inode_id_new, parent, name, full_path.clone(), kind);
        self.inodes
            .write()
            .unwrap()
            .insert(inode_id_new, inode_new.clone());
        self.path_index
            .write()
            .unwrap()
            .insert(full_path, inode_id_new);
        Ok(inode_new)
    }
}

#[derive(Clone)]
pub struct Inode {
    inner: Arc<InodeInner>,
}

#[allow(dead_code)]
struct InodeInner {
    ino: InodeNo,
    parent: InodeNo,
    name: String,
    full_path: String,
    kind: FileType,
}

#[allow(dead_code)]
impl Inode {
    pub fn ino(&self) -> InodeNo {
        self.inner.ino
    }

    pub fn parent(&self) -> InodeNo {
        self.inner.parent
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn full_path(&self) -> &str {
        &self.inner.full_path
    }
    pub fn kind(&self) -> FileType {
        self.inner.kind
    }

    fn new(ino: InodeNo, parent: InodeNo, name: String, full_path: String, kind: FileType) -> Self {
        let inner = InodeInner {
            ino,
            parent,
            name,
            full_path,
            kind,
        };
        Self {
            inner: inner.into(),
        }
    }
}

impl From<Inode> for FileAttr {
    fn from(value: Inode) -> Self {
        FileAttr {
            ino: value.ino(),
            size: 0,
            blocks: 0,
            atime: UNIX_EPOCH,
            mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind: value.kind(),
            perm: 0o555,
            nlink: 2,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
            blksize: 0,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum InodeError {
    #[error("inode {0} does not exist")]
    InodeDoesNotExist(InodeNo),
    #[error("inode {0} insert failed")]
    InodeInsertFailure(InodeNo),
}
