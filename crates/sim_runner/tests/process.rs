//! Process execution edge cases for Issue #64 §H-8/§J.
//!
//! The remaining §H/§J cases live in `runner.rs`; this file adds the cases that
//! were still missing there.
use sim_runner::results::{RESULTS_CSV, RESULTS_META};
use sim_runner::*;
use std::path::Path;
use std::time::{Duration, Instant};

fn env() -> PythonEnv {
    discover_python(&RunnerConfig::default()).expect("python")
}

fn script(dir: &Path, body: &str) -> std::path::PathBuf {
    let p = dir.join("s.py");
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn poll_events_starts_with_started_and_ends_with_finished() {
    let d = tempfile::tempdir().unwrap();
    let mut h = RunHandle::start(
        &env(),
        &script(
            d.path(),
            "print('SIMPYLINK:START'); print('hello'); print('SIMPYLINK:DONE samples=0')",
        ),
        d.path(),
    )
    .unwrap();
    let mut events = Vec::new();
    let t = Instant::now();
    while !h.is_finished() && t.elapsed() < Duration::from_secs(10) {
        events.extend(h.poll_events());
        std::thread::sleep(Duration::from_millis(20));
    }
    events.extend(h.poll_events());
    assert_eq!(events.first(), Some(&RunEvent::Started));
    assert!(
        matches!(events.last(), Some(RunEvent::Finished(o)) if o.status == RunStatus::Succeeded),
        "{events:?}"
    );
}

#[test]
fn missing_python_program_is_a_spawn_error() {
    let d = tempfile::tempdir().unwrap();
    let env = PythonEnv {
        program: "/definitely/missing/python".into(),
        prefix_args: vec![],
        version: "3.12.0".into(),
    };
    assert!(matches!(
        RunHandle::start(&env, &script(d.path(), "print('x')"), d.path()),
        Err(RunnerError::Spawn(_))
    ));
}

#[test]
fn missing_script_fails_with_python_exit_two() {
    let d = tempfile::tempdir().unwrap();
    let out = run_blocking(&env(), &d.path().join("does_not_exist.py"), d.path(), None).unwrap();
    assert!(
        matches!(
            out.status,
            RunStatus::Failed {
                exit_code: Some(2),
                ..
            }
        ),
        "{:?}",
        out.status
    );
}

#[test]
fn a_failed_run_never_exposes_the_previous_results() {
    // §207-H-1: the out-dir is reused, and the new run fails, so the old results
    // must not be readable afterwards.
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join(RESULTS_META), "stale").unwrap();
    std::fs::write(d.path().join(RESULTS_CSV), "stale").unwrap();
    let s = script(d.path(), "import sys\nprint('boom')\nsys.exit(1)\n");
    let out = run_blocking(&env(), &s, d.path(), None).unwrap();
    assert!(
        matches!(out.status, RunStatus::Failed { .. }),
        "{:?}",
        out.status
    );
    assert!(
        matches!(load_results(d.path()), Err(ResultsError::Missing(_))),
        "the failed run must not leave the previous results behind"
    );
}

#[test]
fn a_runaway_script_is_killed_by_the_timeout() {
    // §207-H-2 / §207-I-2: `--timeout 1` must end a script that never finishes.
    let d = tempfile::tempdir().unwrap();
    let s = script(d.path(), "while True:\n    pass\n");
    let started = Instant::now();
    let out = run_blocking(&env(), &s, d.path(), Some(Duration::from_secs(1))).unwrap();
    assert_eq!(out.status, RunStatus::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "took {:?}",
        started.elapsed()
    );
}
