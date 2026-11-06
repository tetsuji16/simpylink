//! End-to-end helpers: model → IR → Python → run → results.
use sim_codegen_py::{generate, CodegenOptions};
use sim_core::model::Model;
use sim_runner::{
    discover_python, load_results, run_blocking, RunStatus, RunnerConfig, SimResults,
};

/// Runs a model end to end.
pub fn simulate_model(model: &Model) -> Result<SimResults, String> {
    let compiled = sim_ir::compile(model).map_err(|f| {
        f.diagnostics
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let code = generate(&compiled.ir, &CodegenOptions::default()).map_err(|e| e.to_string())?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let script = dir.path().join(&code.file_name);
    std::fs::write(&script, &code.source).map_err(|e| e.to_string())?;
    let env = discover_python(&RunnerConfig::default()).map_err(|e| e.to_string())?;
    let out = run_blocking(&env, &script, dir.path(), None).map_err(|e| e.to_string())?;
    if out.status != RunStatus::Succeeded {
        return Err(format!("{:?}\n{}", out.status, out.stderr.join("\n")));
    }
    load_results(dir.path()).map_err(|e| e.to_string())
}

/// Runs a fixture end to end (panics on failure).
pub fn simulate_fixture(name: &str) -> SimResults {
    simulate_model(&crate::load_fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Element-wise comparison with an absolute tolerance.
pub fn assert_close(actual: &[f64], expected: &[f64], abs_tol: f64) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "length differs: {actual:?} vs {expected:?}"
    );
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (a - e).abs() <= abs_tol,
            "index {i}: actual {a}, expected {e} (tol {abs_tol}); all: {actual:?}"
        );
    }
}
