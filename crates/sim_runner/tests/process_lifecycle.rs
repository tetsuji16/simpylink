//! Process lifecycle tests for Issue #206 §H/§I/§J.
//!
//! The child and its grandchildren must be gone within three seconds of a
//! `stop()` or of dropping the handle. These tests need Python 3.10+, like the
//! other runner tests.
use sim_runner::*;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn env() -> PythonEnv {
    discover_python(&RunnerConfig::default()).expect("python")
}

fn script(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    let p = dir.join("s.py");
    std::fs::write(&p, body).unwrap();
    p
}

/// True while a process with `pid` exists.
fn pid_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let filter = format!("PID eq {pid}");
        let out = Command::new("tasklist")
            .args(["/FI", &filter, "/NH", "/FO", "CSV"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();
        match out {
            Ok(o) => String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")),
            Err(_) => false,
        }
    }
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Waits up to `limit` for the process to disappear.
fn wait_until_dead(pid: u32, limit: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !pid_alive(pid)
}

/// Waits for the next stdout line and parses it as a pid.
fn next_pid(handle: &mut RunHandle, limit: Duration) -> u32 {
    let started = Instant::now();
    while started.elapsed() < limit {
        match handle.wait_event(Duration::from_secs(1)) {
            Some(RunEvent::Stdout(line)) => {
                if let Ok(pid) = line.trim().parse() {
                    return pid;
                }
            }
            Some(_) => {}
            None => {}
        }
    }
    panic!("no pid on stdout within {limit:?}");
}

/// Waits for the Finished event.
fn next_outcome(handle: &mut RunHandle, limit: Duration) -> RunOutcome {
    let started = Instant::now();
    while started.elapsed() < limit {
        if let Some(RunEvent::Finished(o)) = handle.wait_event(Duration::from_secs(1)) {
            return o;
        }
    }
    panic!("no Finished event within {limit:?}");
}

#[test]
fn dropping_the_handle_kills_the_child() {
    let d = tempfile::tempdir().unwrap();
    let mut h = RunHandle::start(
        &env(),
        &script(
            d.path(),
            "import os, time\nprint(os.getpid(), flush=True)\ntime.sleep(60)",
        ),
        d.path(),
    )
    .unwrap();
    let pid = next_pid(&mut h, Duration::from_secs(10));
    assert!(pid_alive(pid), "child {pid} is not running");
    let started = Instant::now();
    drop(h);
    assert!(
        wait_until_dead(pid, Duration::from_secs(3)),
        "child {pid} survived the drop"
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "drop blocked for {:?}",
        started.elapsed()
    );
}

#[test]
fn stop_kills_the_child_and_its_grandchild() {
    let d = tempfile::tempdir().unwrap();
    let mut h = RunHandle::start(
        &env(),
        &script(
            d.path(),
            concat!(
                "import os, subprocess, sys, time\n",
                "gc = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])\n",
                "print(gc.pid, flush=True)\n",
                "print(os.getpid(), flush=True)\n",
                "time.sleep(60)\n",
            ),
        ),
        d.path(),
    )
    .unwrap();
    let grandchild = next_pid(&mut h, Duration::from_secs(10));
    let child = next_pid(&mut h, Duration::from_secs(10));
    assert!(
        pid_alive(grandchild),
        "grandchild {grandchild} is not running"
    );
    assert!(pid_alive(child), "child {child} is not running");
    let started = Instant::now();
    h.stop();
    h.stop();
    assert!(
        wait_until_dead(child, Duration::from_secs(3)),
        "child {child} survived stop()"
    );
    assert!(
        wait_until_dead(grandchild, Duration::from_secs(3)),
        "grandchild {grandchild} survived stop()"
    );
    let outcome = next_outcome(&mut h, Duration::from_secs(10));
    assert_eq!(outcome.status, RunStatus::Stopped);
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn stop_after_finish_does_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut h = RunHandle::start(
        &env(),
        &script(d.path(), "print('SIMPYLINK:DONE samples=0')"),
        d.path(),
    )
    .unwrap();
    let outcome = next_outcome(&mut h, Duration::from_secs(10));
    assert_eq!(outcome.status, RunStatus::Succeeded);
    h.stop();
    h.stop();
    assert!(h.is_finished());
}
