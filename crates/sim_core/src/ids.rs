//! Identifier newtypes.
use serde::{Deserialize, Serialize};
use std::fmt;

/// Block identifier (Simulink SID). Unique across the whole model, starts at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockId(pub u32);

/// Line identifier. Unique across the whole model, starts at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LineId(pub u32);

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for LineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serde_and_display() {
        assert_eq!(serde_json::to_string(&BlockId(7)).unwrap(), "7");
        assert_eq!(serde_json::from_str::<LineId>("12").unwrap(), LineId(12));
        assert_eq!(BlockId(3).to_string(), "3");
        assert!(BlockId(2) < BlockId(10));
        assert!(serde_json::from_str::<BlockId>("-1").is_err());
        assert!(serde_json::from_str::<BlockId>("1.5").is_err());
    }
}
