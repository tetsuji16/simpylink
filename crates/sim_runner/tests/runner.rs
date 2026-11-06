//! Process and results tests (needs Python 3.10+).
use sim_runner::*;
use std::time::{Duration, Instant};

fn env() -> PythonEnv {
    discover_python(&RunnerConfig::default()).expect("python")
}
fn script(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    let p = dir.join("s.py");
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn environment() {
    let e = env();
    assert!(parse_version(&e.version).unwrap() >= (3, 10, 0));
    let r = check_environment(&e).unwrap();
    assert!(r.numpy_version.is_some() && r.scipy_version.is_some());
    let missing = discover_python(&RunnerConfig {
        python: Some("/definitely/missing/python".into()),
        timeout: None,
    });
    assert!(
        matches!(missing, Err(RunnerError::PythonNotFound { tried }) if tried == vec!["/definitely/missing/python".to_string()])
    );
}

#[test]
fn process_outcomes() {
    let d = tempfile::tempdir().unwrap();
    let e = env();
    let o = run_blocking(
        &e,
        &script(
            d.path(),
            "print('SIMPYLINK:START'); print('hello'); print('SIMPYLINK:DONE samples=0')",
        ),
        d.path(),
        None,
    )
    .unwrap();
    assert_eq!(o.status, RunStatus::Succeeded);
    assert_eq!(
        o.stdout,
        vec!["SIMPYLINK:START", "hello", "SIMPYLINK:DONE samples=0"]
    );
    let o = run_blocking(
        &e,
        &script(
            d.path(),
            "import sys; print('SIMPYLINK:ERROR ValueError: bad', file=sys.stderr); sys.exit(1)",
        ),
        d.path(),
        None,
    )
    .unwrap();
    assert_eq!(
        o.status,
        RunStatus::Failed {
            exit_code: Some(1),
            message: "ValueError: bad".into()
        }
    );
    let o = run_blocking(
        &e,
        &script(d.path(), "import sys; sys.exit(3)"),
        d.path(),
        None,
    )
    .unwrap();
    assert_eq!(
        o.status,
        RunStatus::Failed {
            exit_code: Some(3),
            message: "Python exited with code 3".into()
        }
    );
    let o = run_blocking(
        &e,
        &script(d.path(), "import time; time.sleep(30)"),
        d.path(),
        Some(Duration::from_secs(1)),
    )
    .unwrap();
    assert_eq!(o.status, RunStatus::TimedOut);
    let o = run_blocking(&e, &script(d.path(), "print('日本語')"), d.path(), None).unwrap();
    assert_eq!(o.stdout, vec!["日本語"]);
    let o = run_blocking(
        &e,
        &script(d.path(), "import os; print(os.getcwd())"),
        d.path(),
        None,
    )
    .unwrap();
    assert_eq!(
        std::fs::canonicalize(&o.stdout[0]).unwrap(),
        std::fs::canonicalize(d.path()).unwrap()
    );
}

#[test]
fn stop_running_process() {
    let d = tempfile::tempdir().unwrap();
    let mut h = RunHandle::start(
        &env(),
        &script(
            d.path(),
            "import time\nprint('x', flush=True)\ntime.sleep(30)",
        ),
        d.path(),
    )
    .unwrap();
    let t = Instant::now();
    assert_eq!(
        h.wait_event(Duration::from_secs(10)),
        Some(RunEvent::Started)
    );
    assert_eq!(
        h.wait_event(Duration::from_secs(10)),
        Some(RunEvent::Stdout("x".into()))
    );
    h.stop();
    h.stop();
    let mut status = None;
    while status.is_none() && t.elapsed() < Duration::from_secs(10) {
        if let Some(RunEvent::Finished(o)) = h.wait_event(Duration::from_secs(10)) {
            status = Some(o.status);
        }
    }
    assert_eq!(status, Some(RunStatus::Stopped));
    assert!(h.is_finished());
    assert!(t.elapsed() < Duration::from_secs(10));
}

const META: &str = r#"{"format":"simpylink-results","format_version":1,"model":"m","status":"ok","error":null,"num_samples":5,"channels":[{"name":"Scope/1","block_sid":3,"port":1}]}"#;

#[test]
fn results_loading() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("results.meta.json"), META).unwrap();
    std::fs::write(
        d.path().join("results.csv"),
        "time,Scope/1\r\n0.0,0.0\r\n0.5,nan\r\n1.0,inf\r\n1.5,-inf\r\n2.0,6.0\r\n",
    )
    .unwrap();
    let r = load_results(d.path()).unwrap();
    assert_eq!(r.time, vec![0.0, 0.5, 1.0, 1.5, 2.0]);
    let v = &r.channels[0].values;
    assert!(v[1].is_nan() && v[2] == f64::INFINITY && v[3] == f64::NEG_INFINITY && v[4] == 6.0);
    assert_eq!((r.channels[0].block_sid, r.channels[0].port), (3, 1));
    std::fs::write(d.path().join("results.csv"), "time,Other\n0.0,0.0\n").unwrap();
    assert!(matches!(
        load_results(d.path()),
        Err(ResultsError::Csv { line: 1, .. })
    ));
    std::fs::write(d.path().join("results.csv"), "time,Scope/1\n0.0,0.0\n1.0\n").unwrap();
    assert!(
        matches!(load_results(d.path()), Err(ResultsError::Csv { line: 3, ref message }) if message == "expected 2 columns, found 1")
    );
    std::fs::write(d.path().join("results.csv"), "time,Scope/1\n0.0,abc\n").unwrap();
    assert!(
        matches!(load_results(d.path()), Err(ResultsError::Csv { ref message, .. }) if message == "invalid number 'abc'")
    );
    std::fs::write(d.path().join("results.csv"), "time,Scope/1\n0.0,1.0\n").unwrap();
    assert!(matches!(load_results(d.path()), Err(ResultsError::Meta(_))));
    std::fs::write(
        d.path().join("results.meta.json"),
        META.replace(
            "\"ok\",\"error\":null",
            "\"error\",\"error\":\"SimulationError: x\"",
        ),
    )
    .unwrap();
    assert!(
        matches!(load_results(d.path()), Err(ResultsError::Reported(ref s)) if s == "SimulationError: x")
    );
    std::fs::write(
        d.path().join("results.meta.json"),
        META.replace("\"format_version\":1", "\"format_version\":2"),
    )
    .unwrap();
    assert!(
        matches!(load_results(d.path()), Err(ResultsError::Meta(ref s)) if s == "unsupported results format")
    );
    let e = tempfile::tempdir().unwrap();
    assert!(matches!(
        load_results(e.path()),
        Err(ResultsError::Missing(_))
    ));
}
