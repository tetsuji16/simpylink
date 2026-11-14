//! Python discovery and environment tests (Issue #63, DES-04 §6).
//!
//! These tests exercise a real interpreter. CI sets `SIMPYLINK_PYTHON=python`;
//! locally a Python 3.10+ with NumPy and SciPy must be available.
use sim_runner::{check_environment, discover_python, parse_version, RunnerConfig, RunnerError};

#[test]
fn discovers_python_and_required_packages() {
    let env = discover_python(&RunnerConfig::default())
        .expect("Python 3.10+ is required; set SIMPYLINK_PYTHON to an interpreter");
    let version = parse_version(&env.version).expect("version output must be MAJOR.MINOR.PATCH");
    assert!(
        version >= (3, 10, 0),
        "found Python {} at {}",
        env.version,
        env.program.display()
    );
    let report = check_environment(&env).expect("numpy and scipy must be installed");
    assert!(report.numpy_version.is_some(), "{report:?}");
    assert!(report.scipy_version.is_some(), "{report:?}");
}

#[test]
fn missing_interpreter_is_reported() {
    let missing = discover_python(&RunnerConfig {
        python: Some("/definitely/missing/python".into()),
        timeout: None,
    });
    assert!(
        matches!(
            missing,
            Err(RunnerError::PythonNotFound { ref tried })
                if *tried == vec!["/definitely/missing/python".to_string()]
        ),
        "{missing:?}"
    );
}
