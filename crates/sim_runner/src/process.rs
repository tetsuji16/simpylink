//! Child process execution.
use crate::python::no_window;
use crate::types::*;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Upper bound for the helper process used by [`kill_tree`].
const KILL_HELPER_TIMEOUT: Duration = Duration::from_millis(500);

/// Handle to a running simulation.
pub struct RunHandle {
    child: Arc<Mutex<Option<Child>>>,
    rx: Receiver<RunEvent>,
    finished: bool,
    stop_requested: Arc<AtomicBool>,
}

/// Runs `cmd` and waits at most `limit` for it, then kills the helper itself.
///
/// Returns `true` when the helper ran to completion on its own.
fn run_bounded(mut cmd: Command, limit: Duration) -> bool {
    let Ok(mut helper) = cmd.spawn() else {
        return false;
    };
    let started = Instant::now();
    loop {
        match helper.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => {}
            Err(_) => return false,
        }
        if started.elapsed() >= limit {
            let _killed = helper.kill();
            let _waited = helper.wait();
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Kills `child` and the processes it started.
///
/// Windows uses `taskkill /T`, Unix sends `SIGKILL` to the child's process
/// group (children are spawned with `process_group(0)`, so the pid is also the
/// group id). Both fall back to `Child::kill` when the helper cannot run, and
/// the child is reaped before returning.
pub(crate) fn kill_tree(child: &mut Child) {
    let pid = child.id();
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.arg("/PID")
            .arg(pid.to_string())
            .args(["/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        no_window(&mut cmd);
        let _helper_finished = run_bounded(cmd, KILL_HELPER_TIMEOUT);
    }
    #[cfg(unix)]
    {
        let mut cmd = Command::new("kill");
        cmd.args(["-KILL", &format!("-{pid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _helper_finished = run_bounded(cmd, KILL_HELPER_TIMEOUT);
    }
    let _killed = child.kill();
    let _waited = child.wait();
}

fn status_from(code: Option<i32>, stderr: &[String], stopped: bool, timed_out: bool) -> RunStatus {
    if stopped {
        return RunStatus::Stopped;
    }
    if timed_out {
        return RunStatus::TimedOut;
    }
    if code == Some(0) {
        return RunStatus::Succeeded;
    }
    let message = stderr
        .iter()
        .find_map(|l| l.strip_prefix("SIMPYLINK:ERROR ").map(str::to_string))
        .or_else(|| stderr.iter().rev().find(|l| !l.trim().is_empty()).cloned())
        .unwrap_or_else(|| {
            format!(
                "Python exited with code {}",
                code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
            )
        });
    RunStatus::Failed {
        exit_code: code,
        message,
    }
}

impl RunHandle {
    /// Starts the script without a timeout.
    pub fn start(env: &PythonEnv, script: &Path, out_dir: &Path) -> Result<RunHandle, RunnerError> {
        Self::start_with_timeout(env, script, out_dir, None)
    }

    /// Starts the script.
    pub fn start_with_timeout(
        env: &PythonEnv,
        script: &Path,
        out_dir: &Path,
        timeout: Option<Duration>,
    ) -> Result<RunHandle, RunnerError> {
        // The child runs inside `out_dir`, so resolve relative paths against the
        // caller's working directory first; otherwise a relative `out_dir` would
        // be applied twice (once here and once by the child's working directory).
        let script = std::path::absolute(script)?;
        let out_dir = std::path::absolute(out_dir)?;
        std::fs::create_dir_all(&out_dir)?;
        let mut cmd = Command::new(&env.program);
        cmd.args(&env.prefix_args)
            .arg("-u")
            .arg(&script)
            .arg("--out-dir")
            .arg(&out_dir)
            .current_dir(&out_dir)
            .env("PYTHONIOENCODING", "utf-8")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        no_window(&mut cmd);
        #[cfg(unix)]
        {
            // Own process group, so `kill_tree` can signal the whole tree.
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let mut child = cmd.spawn().map_err(|e| RunnerError::Spawn(e.to_string()))?;
        let started = Instant::now();
        let (tx, rx) = mpsc::channel();
        let _sent = tx.send(RunEvent::Started);
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let txo = tx.clone();
        let out_thread = std::thread::spawn(move || {
            let mut all = vec![];
            if let Some(s) = stdout {
                for line in BufReader::new(s).lines().map_while(Result::ok) {
                    let _sent = txo.send(RunEvent::Stdout(line.clone()));
                    all.push(line);
                }
            }
            all
        });
        let txe = tx.clone();
        let err_thread = std::thread::spawn(move || {
            let mut all = vec![];
            if let Some(s) = stderr {
                for line in BufReader::new(s).lines().map_while(Result::ok) {
                    let _sent = txe.send(RunEvent::Stderr(line.clone()));
                    all.push(line);
                }
            }
            all
        });
        let child = Arc::new(Mutex::new(Some(child)));
        let stop = Arc::new(AtomicBool::new(false));
        let (c2, s2) = (child.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut timed_out = false;
            let code = loop {
                let mut guard = match c2.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                let Some(ch) = guard.as_mut() else { break None };
                match ch.try_wait() {
                    Ok(Some(st)) => break st.code(),
                    Ok(None) => {}
                    Err(_) => break None,
                }
                if let Some(t) = timeout {
                    if started.elapsed() > t {
                        timed_out = true;
                        kill_tree(ch);
                    }
                }
                drop(guard);
                std::thread::sleep(Duration::from_millis(20));
            };
            let out = out_thread.join().unwrap_or_default();
            let err = err_thread.join().unwrap_or_default();
            let status = status_from(code, &err, s2.load(Ordering::SeqCst), timed_out);
            let _sent = tx.send(RunEvent::Finished(RunOutcome {
                status,
                stdout: out,
                stderr: err,
                elapsed: started.elapsed(),
            }));
        });
        Ok(RunHandle {
            child,
            rx,
            finished: false,
            stop_requested: stop,
        })
    }

    /// Non-blocking: all pending events.
    pub fn poll_events(&mut self) -> Vec<RunEvent> {
        let mut v = vec![];
        while let Ok(e) = self.rx.try_recv() {
            if matches!(e, RunEvent::Finished(_)) {
                self.finished = true;
            }
            v.push(e);
        }
        v
    }

    /// Blocks until the next event (or timeout).
    pub fn wait_event(&mut self, timeout: Duration) -> Option<RunEvent> {
        let e = self.rx.recv_timeout(timeout).ok()?;
        if matches!(e, RunEvent::Finished(_)) {
            self.finished = true;
        }
        Some(e)
    }

    /// Kills the process and the processes it started. Safe to call twice.
    pub fn stop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        let mut guard = match self.child.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if let Some(c) = guard.as_mut() {
            kill_tree(c);
        }
    }

    /// True after the Finished event was received.
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

impl Drop for RunHandle {
    /// Kills the child if it is still running. Never blocks for more than ~1 s.
    fn drop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        let mut guard = match self.child.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let Some(child) = guard.as_mut() else {
            return;
        };
        let already_exited = matches!(child.try_wait(), Ok(Some(_)));
        if !already_exited {
            kill_tree(child);
        }
    }
}

/// Runs a script and waits for completion.
pub fn run_blocking(
    env: &PythonEnv,
    script: &Path,
    out_dir: &Path,
    timeout: Option<Duration>,
) -> Result<RunOutcome, RunnerError> {
    let h = RunHandle::start_with_timeout(env, script, out_dir, timeout)?;
    loop {
        match h.rx.recv() {
            Ok(RunEvent::Finished(o)) => return Ok(o),
            Ok(_) => {}
            Err(_) => {
                return Err(RunnerError::Spawn(
                    "runner thread ended unexpectedly".into(),
                ))
            }
        }
    }
}
