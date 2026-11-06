//! Quick insert (double-click on the canvas and type a block name).
use sim_core::geom::Point;
use sim_core::library::{registry, BlockSpec};

/// Popup state.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickInsertState {
    pub at: Point,
    pub query: String,
    pub highlighted: usize,
    pub focus_requested: bool,
}

impl QuickInsertState {
    /// New popup at a model position.
    pub fn new(at: Point) -> Self {
        QuickInsertState {
            at,
            query: String::new(),
            highlighted: 0,
            focus_requested: false,
        }
    }
    /// Matching specs (prefix matches first, then substring), max 10.
    pub fn matches(&self) -> Vec<&'static BlockSpec> {
        search_blocks(&self.query, 10)
    }
    /// Moves the highlight.
    pub fn move_highlight(&mut self, delta: i32) {
        let n = self.matches().len();
        if n == 0 {
            self.highlighted = 0;
            return;
        }
        self.highlighted = (self.highlighted as i64 + delta as i64).clamp(0, n as i64 - 1) as usize;
    }
    /// Highlighted block type.
    pub fn chosen(&self) -> Option<&'static str> {
        self.matches().get(self.highlighted).map(|s| s.block_type)
    }
}

/// Block search used by quick insert and the library browser.
pub fn search_blocks(query: &str, limit: usize) -> Vec<&'static BlockSpec> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return registry().iter().take(limit).collect();
    }
    let names = |s: &BlockSpec| {
        [
            s.display_name.to_lowercase(),
            s.block_type.to_lowercase(),
            s.base_name.to_lowercase(),
        ]
    };
    let mut out: Vec<&'static BlockSpec> = registry()
        .iter()
        .filter(|s| names(s).iter().any(|n| n.starts_with(&q)))
        .collect();
    for s in registry() {
        if !out.iter().any(|o| o.block_type == s.block_type)
            && names(s).iter().any(|n| n.contains(&q))
        {
            out.push(s);
        }
    }
    out.truncate(limit);
    out
}
