use sim_codegen_py::{generate, CodegenOptions};
use sim_core::diag::codes::W_UNCONNECTED_INPUT;
use sim_core::json::load_from_path;
use sim_core::validate::validate_model;
use sim_integration::e2e::{assert_close, simulate_model};
use std::path::PathBuf;

fn fixture() -> sim_core::model::Model {
    load_from_path(&PathBuf::from(format!(
        "{}/../fixtures/models/ground_terminator.json",
        env!("CARGO_MANIFEST_DIR")
    )))
    .unwrap()
}

#[test]
fn ground_and_terminator_execute_as_expected() {
    let model = fixture();
    let result = simulate_model(&model).unwrap();
    assert_close(&result.time, &[0.0, 0.5, 1.0], 1e-12);
    assert_close(
        &result.channel("Scope/1").unwrap().values,
        &[-2.0; 3],
        1e-12,
    );

    let compiled = sim_ir::compile(&model).unwrap();
    let generated = generate(&compiled.ir, &CodegenOptions::default()).unwrap();
    assert!(!generated.source.contains("# Terminator (SID 5)"));
}

#[test]
fn unconnected_terminator_warns_and_still_compiles() {
    let mut model = fixture();
    model.root.lines[2].branches.clear();

    let diagnostics = validate_model(&model);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == W_UNCONNECTED_INPUT
            && diagnostic.location.block.is_some_and(|sid| sid.0 == 5)
    }));
    assert!(sim_ir::compile(&model).is_ok());
}
