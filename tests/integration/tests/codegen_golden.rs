//! Golden tests: generated Python must match the expected files byte for byte.
use sim_codegen_py::{generate, CodegenError, CodegenOptions};
use sim_integration::{assert_text_eq, fixtures_dir, load_fixture};

fn gen(name: &str) -> String {
    let ir = sim_ir::compile(&load_fixture(name))
        .unwrap_or_else(|f| panic!("{name}: {:?}", f.diagnostics))
        .ir;
    generate(
        &ir,
        &CodegenOptions {
            generator_version: "0.1.0".into(),
        },
    )
    .unwrap()
    .source
}

#[test]
fn goldens() {
    for name in [
        "step_gain_scope",
        "sum_constants",
        "const_integrator",
        "first_order_lag",
        "accumulator",
        "gain_constant",
    ] {
        let expected =
            std::fs::read_to_string(fixtures_dir().join("expected").join(format!("{name}.py")))
                .unwrap();
        let actual = gen(name);
        assert_text_eq(&actual, &expected, name);
        assert!(!actual.contains('\r'), "{name}: contains a carriage return");
        assert!(!actual.contains('\t'), "{name}: contains a tab");
        assert!(
            actual.lines().all(|l| !l.ends_with(' ')),
            "{name}: contains trailing spaces"
        );
    }
}

#[test]
fn file_name_is_derived_from_model_name() {
    let ir = sim_ir::compile(&load_fixture("step_gain_scope"))
        .unwrap()
        .ir;
    let code = generate(&ir, &CodegenOptions::default()).unwrap();
    assert_eq!(code.file_name, "step_gain_scope.py");
}

#[test]
fn nan_gain_is_rejected() {
    let mut ir = sim_ir::compile(&load_fixture("step_gain_scope"))
        .unwrap()
        .ir;
    let gain = ir
        .blocks
        .iter_mut()
        .find(|b| matches!(b.kind, sim_ir::IrBlockKind::Gain { .. }))
        .expect("step_gain_scope contains a Gain block");
    gain.kind = sim_ir::IrBlockKind::Gain { gain: f64::NAN };
    match generate(&ir, &CodegenOptions::default()) {
        Err(CodegenError::InvalidNumber(msg)) => assert!(msg.contains("NaN"), "{msg}"),
        other => panic!("expected InvalidNumber, got {other:?}"),
    }
}

#[test]
fn model_name_quotes_are_escaped() {
    let mut model = load_fixture("gain_constant");
    model.name = "my\"model\\dir".into();
    let ir = sim_ir::compile(&model).unwrap().ir;
    let src = generate(&ir, &CodegenOptions::default()).unwrap().source;
    assert!(
        src.contains("Model: my'model\\\\dir"),
        "docstring model name is not escaped:\n{src}"
    );
    assert!(
        src.contains("MODEL_NAME = \"my\\\"model\\\\dir\""),
        "MODEL_NAME is not escaped:\n{src}"
    );
}

#[test]
fn quoted_model_name_compiles() {
    let mut model = load_fixture("gain_constant");
    model.name = "my\"model\\dir".into();
    let ir = sim_ir::compile(&model).unwrap().ir;
    let code = generate(&ir, &CodegenOptions::default()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(&code.file_name);
    std::fs::write(&path, &code.source).unwrap();
    let env = sim_runner::discover_python(&sim_runner::RunnerConfig::default())
        .expect("Python 3.10+ is required for the generated-code tests");
    let output = std::process::Command::new(&env.program)
        .args(&env.prefix_args)
        .arg("-m")
        .arg("py_compile")
        .arg(&path)
        .output()
        .expect("failed to start Python");
    assert!(
        output.status.success(),
        "py_compile failed for {}:\n{}",
        code.file_name,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn deterministic_and_clean() {
    let a = gen("pi_loop");
    assert_eq!(a, gen("pi_loop"));
    assert!(!a.contains('\r') && !a.contains('\t'));
    assert!(a.lines().all(|l| !l.ends_with(' ')));
}

#[test]
fn unconnected_input_reads_zero() {
    let mut m = load_fixture("sum_constants");
    m.root.lines.retain(|l| l.id.0 != 2);
    let ir = sim_ir::compile(&m).unwrap().ir;
    let src = generate(&ir, &CodegenOptions::default()).unwrap().source;
    assert!(src.contains("    sum_3 = constant_1 - 0.0\n"));
}
