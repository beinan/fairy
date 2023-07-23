use fuser::{Filesystem, MountOption};
use std::ffi::{OsStr, OsString};
use std::path::Path;
use crate::async_fuse::fusemt::FuseMT;
use crate::passthrough::passthrough::PassthroughFS;

mod passthrough;
mod async_fuse;
mod fuser;

struct FairyFS;

impl Filesystem for FairyFS {

}

pub fn mount(mountpoint: &Path) {
    fuser::mount2(FairyFS, mountpoint, &[MountOption::AutoUnmount]).unwrap();
}

pub fn mount_passthrough(mountpoint: &Path, source: &Path) {
    let filesystem = PassthroughFS {
        target: OsString::from(source.as_os_str()),
    };

    let fuse_args = [OsStr::new("-o"), OsStr::new("fsname=passthrufs")];

    async_fuse::mount(FuseMT::new(filesystem, 1), mountpoint, &fuse_args[..]).unwrap();
}