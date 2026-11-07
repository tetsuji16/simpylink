//! CLI tests.
use clap::Parser;
use sim_app::cli::{Cli, CliCommand};
use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_simpylink"))
}
fn fx(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/models")
        .join(format!("{name}.json"))
}

#[test]
fn parsing() {
    assert_eq!(Cli::try_parse_from(["simpylink"]).unwrap().command, None);
    assert_eq!(
        Cli::try_parse_from(["simpylink", "codegen", "m.json", "-o", "out.py"])
            .unwrap()
            .command,
        Some(CliCommand::Codegen {
            model: "m.json".into(),
            output: "out.py".into()
        })
    );
    assert_eq!(
        Cli::try_parse_from([
            "simpylink",
            "run",
            "m.json",
            "--out-dir",
            "r",
            "--python",
            "/usr/bin/python3",
            "--keep-script"
        ])
        .unwrap()
        .command,
        Some(CliCommand::Run {
            model: "m.json".into(),
            out_dir: "r".into(),
            python: Some("/usr/bin/python3".into()),
            keep_script: true
        })
    );
    assert_eq!(
        Cli::try_parse_from(["simpylink", "check-python"])
            .unwrap()
            .command,
        Some(CliCommand::CheckPython { python: None })
    );
    assert!(Cli::try_parse_from(["simpylink", "codegen", "m.json"]).is_err());
    assert!(Cli::try_parse_from(["simpylink", "unknown"]).is_err());
}

#[test]
fn codegen_command() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("sub/dir/a.py");
    let o = bin()
        .args(["codegen"])
        .arg(fx("step_gain_scope"))
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let got = std::fs::read_to_string(&out).unwrap();
    let want = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/expected/step_gain_scope.py"),
    )
    .unwrap();
    let strip = |s: &str| {
        s.lines()
            .filter(|l| !l.starts_with("Generator:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&got), strip(&want));
    let b = d.path().join("b.py");
    let o = bin()
        .args(["codegen"])
        .arg(fx("algebraic_loop"))
        .arg("-o")
        .arg(&b)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&o.stderr).contains(
        "error[E_ALGEBRAIC_LOOP]: algebraic loop: Gain(2) -> Sum(3) -> Gain(2) (block 2 \"Gain\")"
    ));
    assert!(!b.exists());
    let o = bin()
        .args(["codegen", "nope.json", "-o", "x.py"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(4));
}

#[test]
fn run_command() {
    let d = tempfile::tempdir().unwrap();
    let o = bin()
        .arg("run")
        .arg(fx("step_gain_scope"))
        .arg("--out-dir")
        .arg(d.path())
        .output()
        .unwrap();
    let so = String::from_utf8_lossy(&o.stdout);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{so}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(so.contains("simulation finished: 5 samples, t = 0 .. 2"));
    assert!(so.contains("  Scope/1: min 0, max 6, final 6"));
    assert!(d.path().join("results.csv").exists());
    assert!(!d.path().join("step_gain_scope.py").exists());
    let o = bin()
        .arg("run")
        .arg(fx("step_gain_scope"))
        .arg("--out-dir")
        .arg(d.path())
        .arg("--keep-script")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(d.path().join("step_gain_scope.py").exists());
    let o = bin()
        .arg("run")
        .arg(fx("algebraic_loop"))
        .arg("--out-dir")
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(3));
    let o = bin()
        .arg("run")
        .arg(fx("step_gain_scope"))
        .arg("--out-dir")
        .arg(d.path())
        .args(["--python", "/missing/python"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(5));
    let o = bin().arg("check-python").output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&o.stdout).lines().count(), 3);
}

#[test]
fn run_with_relative_out_dir() {
    let d = tempfile::tempdir().unwrap();
    let o = bin()
        .arg("run")
        .arg(fx("step_gain_scope"))
        .args(["--out-dir", "results"])
        .current_dir(d.path())
        .output()
        .unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(o.status.code(), Some(0), "{combined}");
    assert!(d.path().join("results/results.csv").exists());
    assert!(!d.path().join("results/results").exists());
}
