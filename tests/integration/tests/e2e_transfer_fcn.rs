//! Transfer Fcn parameter errors (Issue #73 §J).
use sim_core::model::Model;
use sim_integration::e2e::simulate_model;
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
fn improper_and_zero_denominators_are_param_domain_errors() {
    let mut m = load_fixture("tf_first_order");
    set(&mut m, 2, "Numerator", "[1 2 3]");
    set(&mut m, 2, "Denominator", "[1 1]");
    let err = simulate_model(&m).unwrap_err();
    assert!(err.contains("E_PARAM_DOMAIN"), "{err}");
    assert!(err.contains("must be proper"), "{err}");

    let mut m = load_fixture("tf_first_order");
    set(&mut m, 2, "Denominator", "0");
    let err = simulate_model(&m).unwrap_err();
    assert!(err.contains("E_PARAM_DOMAIN"), "{err}");
    assert!(err.contains("denominator must not be zero"), "{err}");
}

#[test]
fn unevaluable_denominator_is_invalid_param() {
    let mut m = load_fixture("tf_first_order");
    set(&mut m, 2, "Denominator", "[1 K]");
    let err = simulate_model(&m).unwrap_err();
    assert!(err.contains("E_INVALID_PARAM"), "{err}");
}
