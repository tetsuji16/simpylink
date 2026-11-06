//! Path to a (sub)system.
use crate::ids::BlockId;
use serde::{Deserialize, Serialize};

/// SIDs of subsystem blocks from the root to the target system. Root = empty.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SystemPath(pub Vec<BlockId>);

impl SystemPath {
    /// The root system.
    pub fn root() -> Self {
        SystemPath(vec![])
    }
    /// True for the root system.
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
    /// Path of a child subsystem.
    pub fn child(&self, sid: BlockId) -> SystemPath {
        let mut v = self.0.clone();
        v.push(sid);
        SystemPath(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_basics() {
        let p = SystemPath::root();
        assert!(p.is_root());
        let c = p.child(BlockId(5));
        assert_eq!(c.0, vec![BlockId(5)]);
        assert!(!c.is_root());
        assert_eq!(serde_json::to_string(&c).unwrap(), "[5]");
    }
}
