use fuser::{Filesystem, MountOption};
use std::env;

struct NullFS;

impl Filesystem for NullFS {}

pub fn mount() {
    let mountpoint = "/var/mnt";
    fuser::mount2(NullFS, mountpoint, &[MountOption::AutoUnmount]).unwrap();
}