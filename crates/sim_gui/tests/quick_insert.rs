//! Quick insert popup tests (M1-13, issue #54).
//!
//! Note: section H of the issue was written when the registry had 7 blocks.
//! The registry now contains the M4/M6 blocks as well, so the exact lists are
//! asserted against the current registry while the rules (prefix-first order,
//! trim + lowercase, clamping, `None` on no match) follow the issue.

use sim_core::geom::Point;
use sim_gui::dialogs::quick_insert::{search_blocks, QuickInsertState};

fn types(v: &[&'static sim_core::library::BlockSpec]) -> Vec<&'static str> {
    v.iter().map(|s| s.block_type).collect()
}

#[test]
fn empty_query_returns_registry_order_with_limit() {
    let got = types(&search_blocks("", 10));
    assert_eq!(got.len(), 10);
    assert_eq!(
        &got[..7],
        &["Constant", "Step", "Gain", "Sum", "Scope", "Inport", "Outport"]
    );
}

#[test]
fn prefix_matches_come_before_substring_matches() {
    // The M6 registry has more than ten "s..." blocks, so limit 10 is filled by
    // prefix matches alone and no substring match can appear.
    let got = types(&search_blocks("s", 10));
    assert!(
        got.iter().all(|t| t.to_lowercase().starts_with('s')),
        "got {got:?}"
    );

    // With a limit large enough to reach them, the substring matches follow.
    let all = types(&search_blocks("s", 1000));
    let first_substring = all
        .iter()
        .position(|t| !t.to_lowercase().starts_with('s'))
        .unwrap_or(all.len());
    assert!(first_substring >= 5, "got {all:?}");
    for t in &all[first_substring..] {
        assert!(
            t.to_lowercase().contains('s'),
            "{t} should be a substring match"
        );
    }
    assert!(all.contains(&"Constant"), "got {all:?}");
}

#[test]
fn ga_finds_gain() {
    assert_eq!(types(&search_blocks("ga", 10)), vec!["Gain"]);
}

#[test]
fn out_finds_outport_first() {
    let got = types(&search_blocks("out", 10));
    assert_eq!(got.first().copied(), Some("Outport"), "got {got:?}");
}

#[test]
fn unknown_query_is_empty() {
    assert!(search_blocks("zzz", 10).is_empty());
    let mut st = QuickInsertState::new(Point::new(0.0, 0.0));
    st.query = "zzz".into();
    assert_eq!(st.chosen(), None);
}

#[test]
fn query_is_trimmed_and_lowercased() {
    assert_eq!(types(&search_blocks(" GAIN ", 10)), vec!["Gain"]);
}

#[test]
fn highlight_moves_and_clamps() {
    let mut st = QuickInsertState::new(Point::new(10.0, 20.0));
    st.query = "s".into();
    let n = st.matches().len();
    assert!(n > 1);
    st.move_highlight(1000);
    assert_eq!(st.highlighted, n - 1);
    st.move_highlight(-1000);
    assert_eq!(st.highlighted, 0);
    assert_eq!(st.chosen(), Some(st.matches()[0].block_type));
}

#[test]
fn highlight_resets_when_nothing_matches() {
    let mut st = QuickInsertState::new(Point::new(0.0, 0.0));
    st.query = "zzz".into();
    st.highlighted = 5;
    st.move_highlight(1);
    assert_eq!(st.highlighted, 0);
}
