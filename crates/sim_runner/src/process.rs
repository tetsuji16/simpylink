//! Child process execution.
use crate::python::no_window;
use crate::types::*;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Handle to a running simulation.
pub struct RunHandle {
    child: Arc<Mutex<Option<Child>>>,
    rx: Receiver<RunEvent>,
    finished: bool,
    stop_requested: Arc<AtomicBool>,
}

/// Most lines of stdout (and, separately, of stderr) kept for `RunOutcome`.
///
/// A runaway script can print without end; only the newest lines are kept so memory
/// stays bounded. The newest lines are the ones that explain a failure.
pub const MAX_RETAINED_LINES: usize = 10_000;

/// The newest `MAX_RETAINED_LINES` lines of one output stream.
#[derive(Default)]
struct RetainedLines {
    lines: VecDeque<String>,
}

impl RetainedLines {
    fn push(&mut self, line: String) {
        if self.lines.len() == MAX_RETAINED_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }
    fn into_vec(self) -> Vec<String> {
        self.lines.into()
    }
}

/// Reads `reader` to its end and calls `on_line` for every line (without `\n` / `\r\n`).
///
/// Bytes that are not valid UTF-8 are replaced by U+FFFD instead of ending the read:
/// if the reader stopped early the pipe would fill up and the child would block forever.
fn drain_lines(reader: impl Read, mut on_line: impl FnMut(String)) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            // End of stream, or the pipe is broken: nothing more can arrive.
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if buf.last() == Some(&b'\n') {
            buf.pop();
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
        }
        on_line(String::from_utf8_lossy(&buf).into_owned());
    }
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
        let mut child = cmd.spawn().map_err(|e| RunnerError::Spawn(e.to_string()))?;
        let started = Instant::now();
        let (tx, rx) = mpsc::channel();
        let _sent = tx.send(RunEvent::Started);
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let txo = tx.clone();
        let out_thread = std::thread::spawn(move || {
            let mut all = RetainedLines::default();
            if let Some(s) = stdout {
                drain_lines(s, |line| {
                    let _sent = txo.send(RunEvent::Stdout(line.clone()));
                    all.push(line);
                });
            }
            all.into_vec()
        });
        let txe = tx.clone();
        let err_thread = std::thread::spawn(move || {
            let mut all = RetainedLines::default();
            if let Some(s) = stderr {
                drain_lines(s, |line| {
                    let _sent = txe.send(RunEvent::Stderr(line.clone()));
                    all.push(line);
                });
            }
            all.into_vec()
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
                        let _killed = ch.kill();
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

    /// Kills the process.
    pub fn stop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        let mut guard = match self.child.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if let Some(c) = guard.as_mut() {
            let _killed = c.kill();
        }
    }

    /// True after the Finished event was received.
    pub fn is_finished(&self) -> bool {
        self.finished
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

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_of(bytes: &[u8]) -> Vec<String> {
        let mut v = vec![];
        drain_lines(bytes, |l| v.push(l));
        v
    }

    #[test]
    fn drain_lines_splits_and_trims_line_ends() {
        assert_eq!(lines_of(b"a\nb\r\n\nlast"), ["a", "b", "", "last"]);
        assert_eq!(lines_of(b""), Vec::<String>::new());
    }

    #[test]
    fn drain_lines_keeps_reading_after_invalid_utf8() {
        assert_eq!(
            lines_of(b"ok\n\xff\xfe bad\nafter\n"),
            ["ok", "\u{fffd}\u{fffd} bad", "after"]
        );
    }

    #[test]
    fn retained_lines_keep_the_newest() {
        let mut r = RetainedLines::default();
        for i in 0..MAX_RETAINED_LINES + 5 {
            r.push(i.to_string());
        }
        let v = r.into_vec();
        assert_eq!(v.len(), MAX_RETAINED_LINES);
        assert_eq!(v[0], "5");
        assert_eq!(v[v.len() - 1], (MAX_RETAINED_LINES + 4).to_string());
    }
}
