//! Process execution edge cases for Issue #64 §H-8/§J.
//!
//! The remaining §H/§J cases live in `runner.rs`; this file adds the cases that
//! were still missing there.
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
fn fast_output_keeps_the_queue_bounded_and_still_finishes() {
    let d = tempfile::tempdir().unwrap();
    let lines = 50_000;
    let mut h = RunHandle::start(
        &env(),
        &script(
            d.path(),
            &format!("for i in range({lines}):\n    print('line', i)\n"),
        ),
        d.path(),
    )
    .unwrap();
    // Poll nothing for a while: the pipes must still be drained, otherwise the child
    // blocks on a full pipe and the run never ends.
    let t = Instant::now();
    while h.dropped_events() == 0 && t.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        h.dropped_events() > 0,
        "the pending-event queue should have reached its limit"
    );

    let mut events = Vec::new();
    let t = Instant::now();
    while !h.is_finished() && t.elapsed() < Duration::from_secs(30) {
        let batch = h.poll_events();
        // One poll drains a bounded batch even while the child keeps printing.
        assert!(batch.len() <= sim_runner::process::MAX_PENDING_EVENTS);
        events.extend(batch);
        std::thread::sleep(Duration::from_millis(20));
    }
    events.extend(h.poll_events());

    assert_eq!(events.first(), Some(&RunEvent::Started));
    let Some(RunEvent::Finished(o)) = events.last() else {
        panic!("no Finished event after {lines} lines");
    };
    assert_eq!(o.status, RunStatus::Succeeded);
    // Dropping preview events must not lose the outcome: it keeps the newest lines.
    assert_eq!(o.stdout.len(), sim_runner::process::MAX_RETAINED_LINES);
    let last_line = format!("line {}", lines - 1);
    assert_eq!(
        o.stdout.last().map(String::as_str),
        Some(last_line.as_str())
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
