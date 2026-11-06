//! Golden tests: generated Python must match the expected files byte for byte.
use sim_codegen_py::{generate, CodegenOptions};
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
        assert_text_eq(&gen(name), &expected, name);
    }
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
