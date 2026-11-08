//! Integrator E2E tests (Issue #71 §H/§J).
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
fn integrator_initial_condition_and_unconnected_input() {
    let mut m = load_fixture("const_integrator");
    set(&mut m, 2, "InitialCondition", "[1 2]");
    assert!(simulate_model(&m).unwrap_err().contains("E_INVALID_PARAM"));

    let mut m = load_fixture("const_integrator");
    set(&mut m, 2, "InitialCondition", "2.5");
    m.root.lines.retain(|l| l.id != sim_core::ids::LineId(1));
    let compiled = sim_ir::compile(&m).expect("warnings only");
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|d| d.code == "W_UNCONNECTED_INPUT" && d.message.contains("Integrator")),
        "{:?}",
        compiled.diagnostics
    );
    assert_close(
        &simulate_model(&m).unwrap().channels[0].values,
        &[2.5, 2.5, 2.5, 2.5, 2.5],
        1e-9,
    );
}
