use std::ffi::CStr;
use std::{fs::File, io};

pub use fuse_pure::Mount;
pub use mount_options::MountOption;

mod fuse_pure;
pub mod mount_options;

#[inline]
fn libc_umount(mnt: &CStr) -> io::Result<()> {
    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "bitrig",
        target_os = "netbsd"
    ))]
    let r = unsafe { libc::unmount(mnt.as_ptr(), 0) };

    #[cfg(not(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "bitrig",
        target_os = "netbsd"
    )))]
    let r = unsafe { libc::umount(mnt.as_ptr()) };
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn is_mounted(fuse_device: &File) -> bool {
    use libc::{poll, pollfd};
    use std::os::unix::prelude::AsRawFd;

    let mut poll_result = pollfd {
        fd: fuse_device.as_raw_fd(),
        events: 0,
        revents: 0,
    };
    loop {
        let res = unsafe { poll(&mut poll_result, 1, 0) };
        break match res {
            0 => true,
            1 => (poll_result.revents & libc::POLLERR) != 0,
            -1 => {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                } else {
                    // This should never happen. The fd is guaranteed good as `File` owns it.
                    // According to man poll ENOMEM is the only error code unhandled, so we panic
                    // consistent with rust's usual ENOMEM behaviour.
                    panic!("Poll failed with error {}", err)
                }
            }
            _ => unreachable!(),
        };
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;
//     use std::{ffi::CStr, mem::ManuallyDrop};

//     #[test]
//     fn fuse_args() {
//         with_fuse_args(
//             &[
//                 MountOption::CUSTOM("foo".into()),
//                 MountOption::CUSTOM("bar".into()),
//             ],
//             |args| {
//                 let v: Vec<_> = (0..args.argc)
//                     .map(|n| unsafe {
//                         CStr::from_ptr(*args.argv.offset(n as isize))
//                             .to_str()
//                             .unwrap()
//                     })
//                     .collect();
//                 assert_eq!(*v, ["rust-fuse", "-o", "foo", "-o", "bar"]);
//             },
//         );
//     }
//     fn cmd_mount() -> String {
//         std::str::from_utf8(
//             std::process::Command::new("sh")
//                 .arg("-c")
//                 .arg("mount | grep fuse")
//                 .output()
//                 .unwrap()
//                 .stdout
//                 .as_ref(),
//         )
//         .unwrap()
//         .to_owned()
//     }

//     #[test]
//     fn mount_unmount() {
//         // We use ManuallyDrop here to leak the directory on test failure.  We don't
//         // want to try and clean up the directory if it's a mountpoint otherwise we'll
//         // deadlock.
//         let tmp = ManuallyDrop::new(tempfile::tempdir().unwrap());
//         let (file, mount) = Mount::new(tmp.path(), &[]).unwrap();
//         let mnt = cmd_mount();
//         eprintln!("Our mountpoint: {:?}\nfuse mounts:\n{}", tmp.path(), mnt,);
//         assert!(mnt.contains(&*tmp.path().to_string_lossy()));
//         assert!(is_mounted(&file));
//         drop(mount);
//         let mnt = cmd_mount();
//         eprintln!("Our mountpoint: {:?}\nfuse mounts:\n{}", tmp.path(), mnt,);

//         let detached = !mnt.contains(&*tmp.path().to_string_lossy());
//         // Linux supports MNT_DETACH, so we expect unmount to succeed even if the FS
//         // is busy.  Other systems don't so the unmount may fail and we will still
//         // have the mount listed.  The mount will get cleaned up later.
//         #[cfg(target_os = "linux")]
//         assert!(detached);

//         if detached {
//             // We've detached successfully, it's safe to clean up:
//             std::mem::ManuallyDrop::<_>::into_inner(tmp);
//         }

//         // Filesystem may have been lazy unmounted, so we can't assert this:
//         // assert!(!is_mounted(&file));
//     }
// }
