//! `simpylink run` and `simpylink check-python` binary tests (Issue #66, DES-04 §5).
use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_simpylink"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/models")
        .join(format!("{name}.json"))
}

#[test]
fn run_writes_results_and_removes_the_script() {
    let dir = tempfile::tempdir().unwrap();
    let result = bin()
        .arg("run")
        .arg(fixture("step_gain_scope"))
        .arg("--out-dir")
        .arg(dir.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert_eq!(
        result.status.code(),
        Some(0),
        "{stdout}{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(stdout.contains("simulation finished: 5 samples, t = 0 .. 2"));
    assert!(stdout.contains("  Scope/1: min 0, max 6, final 6"));
    assert!(dir.path().join("results.csv").exists());
    assert!(!dir.path().join("step_gain_scope.py").exists());
}

#[test]
fn keep_script_keeps_the_generated_file() {
    let dir = tempfile::tempdir().unwrap();
    let result = bin()
        .arg("run")
        .arg(fixture("step_gain_scope"))
        .arg("--out-dir")
        .arg(dir.path())
        .arg("--keep-script")
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(dir.path().join("step_gain_scope.py").exists());
}

#[test]
fn algebraic_loop_exits_with_a_validation_failure() {
    let dir = tempfile::tempdir().unwrap();
    let result = bin()
        .arg("run")
        .arg(fixture("algebraic_loop"))
        .arg("--out-dir")
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(3));
}

#[test]
fn check_python_succeeds() {
    let result = bin().arg("check-python").output().unwrap();
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&result.stdout).lines().count(), 3);
}

#[test]
fn run_reports_missing_python() {
    let dir = tempfile::tempdir().unwrap();
    let result = bin()
        .arg("run")
        .arg(fixture("step_gain_scope"))
        .arg("--out-dir")
        .arg(dir.path())
        .args(["--python", "/missing/python"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(5));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("error[E_PYTHON]:"), "{stderr}");
}

#[test]
fn check_python_reports_missing_interpreter() {
    let result = bin()
        .args(["check-python", "--python", "/missing/python"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(5));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("error[E_PYTHON]:"), "{stderr}");
}

#[test]
fn timeout_flag_is_accepted() {
    // §207-E: a generous timeout must not change the outcome of a normal run.
    let dir = tempfile::tempdir().unwrap();
    let result = bin()
        .arg("run")
        .arg(fixture("step_gain_scope"))
        .arg("--out-dir")
        .arg(dir.path())
        .args(["--timeout", "60"])
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn invalid_timeout_is_a_usage_error() {
    // §207-J: `--timeout 0` and `--timeout abc` are rejected by clap (exit code 2).
    for bad in ["0", "-1", "abc", "inf", "nan", "1e300"] {
        let result = bin()
            .args(["run", "m.json", "--out-dir", "r", "--timeout", bad])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "--timeout {bad}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            !stderr.trim().is_empty(),
            "--timeout {bad} must explain itself"
        );
    }
}
