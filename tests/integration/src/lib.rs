//! Shared helpers for integration tests.
use sim_core::model::Model;
use std::path::PathBuf;

pub mod e2e;

/// Absolute path of `tests/fixtures`.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
}

/// Loads `tests/fixtures/models/<name>.json`.
pub fn load_fixture(name: &str) -> Model {
    let p = fixtures_dir().join("models").join(format!("{name}.json"));
    sim_core::json::load_from_path(&p).unwrap_or_else(|e| panic!("cannot load {p:?}: {e}"))
}

/// Shows the first differing line of two texts.
pub fn assert_text_eq(actual: &str, expected: &str, what: &str) {
    if actual == expected {
        return;
    }
    let (a, e): (Vec<&str>, Vec<&str>) = (actual.lines().collect(), expected.lines().collect());
    for i in 0..a.len().max(e.len()) {
        let (x, y) = (a.get(i), e.get(i));
        if x != y {
            panic!(
                "{what}: first difference at line {}:\n  actual:   {:?}\n  expected: {:?}",
                i + 1,
                x,
                y
            );
        }
    }
    panic!("{what}: texts differ only in trailing newlines");
}
