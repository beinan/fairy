// use crate::async_fuse::fusemt::FuseMT;
// use crate::passthrough::passthrough_fs::PassthroughFS;
// use fuser::{Filesystem, MountOption};
use std::ffi::{OsStr, OsString};
use std::path::Path;

mod async_fuse;
mod filesystem;
mod fuser;
mod passthrough;

mod uring_fuse;

struct FairyFS;

impl crate::fuser::Filesystem for FairyFS {}

struct UringFilesystem;
impl crate::uring_fuse::filesystem::Filesystem for UringFilesystem {}

pub fn uring_mount(mountpoint: &Path) {
    uring_fuse::mount(UringFilesystem, mountpoint).unwrap();
}

pub fn mount(mountpoint: &Path) {
    fuser::mount2(FairyFS, mountpoint, &[crate::fuser::MountOption::AutoUnmount]).unwrap();
}

pub fn mount_passthrough(mountpoint: &Path, source: &Path) {
    let filesystem = crate::passthrough::passthrough_fs::PassthroughFS {
        target: OsString::from(source.as_os_str()),
    };

    let fuse_args = [OsStr::new("-o"), OsStr::new("fsname=passthrufs")];

    async_fuse::mount(crate::async_fuse::fusemt::FuseMT::new(filesystem, 1), mountpoint, &fuse_args[..]).unwrap();
}
