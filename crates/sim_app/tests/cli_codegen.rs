//! `simpylink codegen` binary tests (Issue #61, sections H and J).
//!
//! Every case runs the real binary so that the exit code and the stderr text
//! are covered end to end.
use sim_core::ids::LineId;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_simpylink"))
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn model(name: &str) -> PathBuf {
    fixtures().join("models").join(format!("{name}.json"))
}

fn expected(name: &str) -> PathBuf {
    fixtures().join("expected").join(format!("{name}.py"))
}

/// The `Generator:` line embeds the CLI version, which differs from the golden file.
fn strip_generator(s: &str) -> String {
    s.lines()
        .filter(|l| !l.starts_with("Generator:"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn codegen(src: &Path, out: &Path) -> Output {
    bin()
        .arg("codegen")
        .arg(src)
        .arg("-o")
        .arg(out)
        .output()
        .unwrap()
}

/// H-1: a valid model generates the golden file (minus the `Generator:` line).
#[test]
fn h1_generates_golden_file() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("a.py");
    let o = codegen(&model("step_gain_scope"), &out);

    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        String::from_utf8_lossy(&o.stdout).trim_end(),
        format!("wrote {}", out.display())
    );
    let got = std::fs::read_to_string(&out).unwrap();
    let want = std::fs::read_to_string(expected("step_gain_scope")).unwrap();
    assert_eq!(strip_generator(&got), strip_generator(&want));
}

/// H-2: an algebraic loop reports the diagnostic and writes no file.
#[test]
fn h2_algebraic_loop_writes_no_file() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("b.py");
    let o = codegen(&model("algebraic_loop"), &out);

    assert_eq!(o.status.code(), Some(3), "{}", stderr(&o));
    assert!(
        stderr(&o).contains(
            "error[E_ALGEBRAIC_LOOP]: algebraic loop: Gain(2) -> Sum(3) -> Gain(2) (block 2 \"Gain\")"
        ),
        "{}",
        stderr(&o)
    );
    assert!(!out.exists());
}

/// H-3: a missing model file is an I/O error.
#[test]
fn h3_missing_model_file() {
    let d = tempfile::tempdir().unwrap();
    let o = codegen(Path::new("nope.json"), &d.path().join("x.py"));

    assert_eq!(o.status.code(), Some(4), "{}", stderr(&o));
    assert!(stderr(&o).starts_with("error[E_IO]:"), "{}", stderr(&o));
}

/// H-4: missing parent directories are created.
#[test]
fn h4_creates_output_directories() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("sub/dir/c.py");
    let o = codegen(&model("step_gain_scope"), &out);

    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(out.exists());
}

/// H-5: warnings go to stderr and the file is still written.
#[test]
fn h5_warnings_are_reported() {
    let d = tempfile::tempdir().unwrap();
    // sum_constants with the Constant1 -> Sum.2 line removed.
    let mut m = sim_core::json::load_from_path(&model("sum_constants")).unwrap();
    m.root.lines.retain(|l| l.id != LineId(2));
    let src = d.path().join("sum_warn.json");
    sim_core::json::save_to_path(&m, &src).unwrap();

    let out = d.path().join("warn.py");
    let o = codegen(&src, &out);

    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(
        stderr(&o).contains("warning[W_UNCONNECTED_INPUT]"),
        "{}",
        stderr(&o)
    );
    assert!(out.exists());
}

/// J: an output path that is an existing directory is an I/O error.
#[test]
fn j_output_path_is_directory() {
    let d = tempfile::tempdir().unwrap();
    let o = codegen(&model("step_gain_scope"), d.path());

    assert_eq!(o.status.code(), Some(4), "{}", stderr(&o));
    assert!(stderr(&o).starts_with("error[E_IO]:"), "{}", stderr(&o));
}
