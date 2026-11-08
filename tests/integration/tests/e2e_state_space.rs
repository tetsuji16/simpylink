//! State-Space dimension validation and defaults (Issue #74 §H/§J).
use sim_core::model::Model;
use sim_integration::e2e::{assert_close, simulate_model};
use sim_integration::load_fixture;

fn set(m: &mut Model, sid: u32, k: &str, v: &str) {
    m.root
        .blocks
        .iter_mut()
        .find(|b| b.sid.0 == sid)
        .unwrap()
        .params
        .insert(k.into(), v.into());
}

#[test]
fn state_space_dimension_errors_and_defaults() {
    let mut m = load_fixture("ss_first_order");
    set(&mut m, 2, "A", "[1 2]");
    assert!(simulate_model(&m).unwrap_err().contains("A must be square"));

    let mut m = load_fixture("ss_second_order");
    set(&mut m, 2, "B", "[0 1]");
    assert!(simulate_model(&m)
        .unwrap_err()
        .contains("B must be 2x1 (got 1x2)"));

    let mut m = load_fixture("ss_second_order");
    set(&mut m, 2, "C", "[1; 0]");
    assert!(simulate_model(&m)
        .unwrap_err()
        .contains("C must be 1x2 (got 2x1)"));

    let mut m = load_fixture("ss_second_order");
    set(&mut m, 2, "D", "[1 2]");
    assert!(simulate_model(&m)
        .unwrap_err()
        .contains("D must be 1x1 (got 1x2)"));

    let mut m = load_fixture("ss_second_order");
    set(&mut m, 2, "InitialCondition", "[1 2 3]");
    assert!(simulate_model(&m).unwrap_err().contains("InitialCondition"));

    let mut m = load_fixture("ss_first_order");
    for k in ["A", "B", "C", "D"] {
        set(&mut m, 2, k, "1");
    }
    set(&mut m, 1, "Value", "0");
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[0.0; 5],
        1e-12,
    );
}
