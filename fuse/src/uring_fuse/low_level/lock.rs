use super::kernel_interface::fuse_file_lock;

/// A newtype for lock owners
///
/// TODO: Document lock lifecycle and how and when to implement file locking.
///
/// See [Read], [Write], [Release], [Flush], [GetLk], [SetLk], [SetLkW].
///
/// We implement conversion from [LockOwner] to [u64] but not vice-versa
/// because all LockOwners are valid [u64]s, but not vice-versa.  So to produce
/// a [LockOwner] from a [u64] we must be explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serializable", derive(Serialize, Deserialize))]
pub struct LockOwner(pub u64);

impl From<LockOwner> for u64 {
    fn from(fh: LockOwner) -> Self {
        fh.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lock {
    // Unfortunately this can't be a std::ops::Range because Range is not Copy:
    // https://github.com/rust-lang/rfcs/issues/2848
    pub range: (u64, u64),
    // TODO: Make typ an enum
    pub typ: i32,
    pub pid: u32,
}
impl Lock {
    pub(super) fn from_abi(x: &fuse_file_lock) -> Lock {
        Lock {
            range: (x.start, x.end),
            typ: x.typ,
            pid: x.pid,
        }
    }
}
