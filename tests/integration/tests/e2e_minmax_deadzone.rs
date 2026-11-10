//! MinMax and Dead Zone E2E tests (Issue #169 §H/§J).
use sim_core::diag::codes::{E_INVALID_PARAM, E_PARAM_DOMAIN};
use sim_integration::e2e::{assert_close, simulate_model};
use sim_integration::load_fixture;

fn set(m: &mut sim_core::model::Model, sid: u32, k: &str, v: &str) {
    m.root
        .blocks
        .iter_mut()
        .find(|b| b.sid.0 == sid)
        .unwrap()
        .params
        .insert(k.into(), v.into());
}

#[test]
fn minmax_and_deadzone_values() {
    let r = simulate_model(&load_fixture("minmax_deadzone")).unwrap();
    assert_close(&r.time, &[0.0, 0.5, 1.0, 1.5, 2.0], 1e-12);
    assert_close(&r.channels[0].values, &[0.5, 0.5, 1.0, 1.5, 2.0], 1e-12);
    assert_close(
        &r.channels[1].values,
        &[-0.75, -0.25, 0.0, 0.25, 0.75],
        1e-12,
    );
}

/// `min` is the other half of `Function`, and the default `Inputs` is 1 (§E).
#[test]
fn min_with_the_default_single_input_is_the_input() {
    let mut m = load_fixture("minmax_deadzone");
    set(&mut m, 3, "Function", "min");
    set(&mut m, 3, "Inputs", "1");
    // Removing the second input leaves line 2 dangling, so drop it as well.
    m.root.lines.retain(|l| l.id.0 != 2);
    let r = simulate_model(&m).unwrap();
    assert_close(&r.channels[0].values, &[0.0, 0.5, 1.0, 1.5, 2.0], 1e-12);
}

#[test]
fn min_of_two_inputs_picks_the_smaller_one() {
    let mut m = load_fixture("minmax_deadzone");
    set(&mut m, 3, "Function", "min");
    let r = simulate_model(&m).unwrap();
    assert_close(&r.channels[0].values, &[0.0, 0.5, 0.5, 0.5, 0.5], 1e-12);
}

#[test]
fn an_inverted_dead_zone_is_a_domain_error() {
    let mut m = load_fixture("minmax_deadzone");
    set(&mut m, 6, "LowerValue", "1");
    set(&mut m, 6, "UpperValue", "0");
    let err = sim_ir::compile(&m).unwrap_err();
    assert!(
        err.diagnostics.iter().any(|d| d.code == E_PARAM_DOMAIN
            && d.message
                .contains("lower value must not exceed upper value")),
        "{:?}",
        err.diagnostics
    );
}

#[test]
fn zero_inputs_is_rejected() {
    let mut m = load_fixture("minmax_deadzone");
    set(&mut m, 3, "Inputs", "0");
    let err = sim_ir::compile(&m).unwrap_err();
    assert!(
        err.diagnostics
            .iter()
            .any(|d| d.code == E_INVALID_PARAM && d.message.contains("Inputs")),
        "{:?}",
        err.diagnostics
    );
}

#[test]
fn defaults_match_the_reference() {
    let minmax = sim_core::library::spec("MinMax").unwrap();
    assert_eq!(
        (minmax.ports)(&std::collections::BTreeMap::new())
            .unwrap()
            .inputs,
        1
    );
    let params: Vec<(&str, &str)> = minmax.params.iter().map(|p| (p.name, p.default)).collect();
    assert_eq!(params, vec![("Function", "min"), ("Inputs", "1")]);

    let dead_zone = sim_core::library::spec("DeadZone").unwrap();
    let params: Vec<(&str, &str)> = dead_zone
        .params
        .iter()
        .map(|p| (p.name, p.default))
        .collect();
    assert_eq!(params, vec![("LowerValue", "-0.5"), ("UpperValue", "0.5")]);
}
