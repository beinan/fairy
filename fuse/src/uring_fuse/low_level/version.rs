use std::fmt::{Display, self};

/// A newtype for ABI version
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serializable", derive(Serialize, Deserialize))]
pub struct Version(pub u32, pub u32);
impl Version {
    pub fn major(&self) -> u32 {
        self.0
    }
    pub fn minor(&self) -> u32 {
        self.1
    }
}
impl Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.0, self.1)
    }
}