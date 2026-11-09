//! `simpylink codegen` binary tests (Issue #61, DES-04 §5).
use sim_core::ids::LineId;
use sim_core::json::{load_from_path, save_to_path};
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
fn writes_file_and_reports_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("sub/dir/a.py");
    let result = bin()
        .arg("codegen")
        .arg(fixture("step_gain_scope"))
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        format!("wrote {}", output.display())
    );
    assert!(output.exists());
    let got = std::fs::read_to_string(&output).unwrap();
    let expected = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/expected/step_gain_scope.py");
    let want = std::fs::read_to_string(expected).unwrap();
    let strip = |s: &str| {
        s.lines()
            .filter(|l| !l.starts_with("Generator:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&got), strip(&want));
}

#[test]
fn algebraic_loop_is_reported_without_writing_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("b.py");
    let result = bin()
        .arg("codegen")
        .arg(fixture("algebraic_loop"))
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&result.stderr).contains(
        "error[E_ALGEBRAIC_LOOP]: algebraic loop: Gain(2) -> Sum(3) -> Gain(2) (block 2 \"Gain\")"
    ));
    assert!(!output.exists());
}

#[test]
fn emits_warning_for_unconnected_input() {
    let dir = tempfile::tempdir().unwrap();
    let mut model = load_from_path(&fixture("sum_constants")).unwrap();
    model.root.lines.retain(|l| l.id != LineId(2));
    let model_path = dir.path().join("partial.json");
    save_to_path(&model, &model_path).unwrap();
    let output = dir.path().join("partial.py");
    let result = bin()
        .arg("codegen")
        .arg(&model_path)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("warning[W_UNCONNECTED_INPUT]"), "{stderr}");
    assert!(output.exists());
}

#[test]
fn missing_model_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    let result = bin()
        .arg("codegen")
        .arg("nope.json")
        .arg("-o")
        .arg(dir.path().join("x.py"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&result.stderr).contains("error[E_IO]:"));
}

#[test]
fn output_directory_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    let result = bin()
        .arg("codegen")
        .arg(fixture("step_gain_scope"))
        .arg("-o")
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&result.stderr).contains("error[E_IO]:"));
}
