//! Child process execution.
use crate::python::no_window;
use crate::types::*;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Upper bound for the helper process used by [`kill_tree`].
const KILL_HELPER_TIMEOUT: Duration = Duration::from_millis(500);

/// Handle to a running simulation.
pub struct RunHandle {
    child: Arc<Mutex<Option<Child>>>,
    events: Arc<EventQueue>,
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
        // The shell builtin handles a negative pid (a process group) on every
        // POSIX shell, while the `kill` binary does not always do so.
        let mut cmd = Command::new("sh");
        cmd.args(["-c", &format!("kill -s KILL -- -{pid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _helper_finished = run_bounded(cmd, KILL_HELPER_TIMEOUT);
    }
    let _killed = child.kill();
    let _waited = child.wait();
}

/// Most lines of stdout (and, separately, of stderr) kept for `RunOutcome`.
///
/// A runaway script can print without end; only the newest lines are kept so memory
/// stays bounded. The newest lines are the ones that explain a failure.
pub const MAX_RETAINED_LINES: usize = 10_000;
/// Maximum UTF-8 bytes retained for one output line, including its truncation marker.
const MAX_LINE_BYTES: usize = 16 * 1024;
const LINE_TRUNCATION_MARKER: &str = " …[line truncated]";

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

/// Most line events waiting between the child process and the caller.
///
/// A script that prints faster than the caller polls would otherwise grow the queue
/// without bound. Once this many line events are pending, further lines are dropped and
/// counted (`RunHandle::dropped_events`); `Started` and `Finished` do not use these slots
/// and are never dropped, and the newest [`MAX_RETAINED_LINES`] lines still arrive inside
/// `RunOutcome`.
pub const MAX_PENDING_EVENTS: usize = 1024;

/// Queue between the reader threads and the caller.
///
/// Unlike an `mpsc` channel it never blocks the producer: the reader threads must keep
/// draining the child's pipes even when the caller is slow, otherwise the child blocks
/// on a full pipe and the run never finishes.
struct EventQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
}

#[derive(Default)]
struct QueueState {
    events: VecDeque<RunEvent>,
    /// Line events inside `events`; `Started` / `Finished` are not counted.
    pending_lines: usize,
    dropped: u64,
    closed: bool,
}

impl QueueState {
    /// Pops one event, keeping `pending_lines` in step.
    fn pop_front(&mut self) -> Option<RunEvent> {
        let event = self.events.pop_front()?;
        if is_line(&event) {
            self.pending_lines = self.pending_lines.saturating_sub(1);
        }
        Some(event)
    }
}

/// True for the per-line events that [`MAX_PENDING_EVENTS`] limits.
fn is_line(event: &RunEvent) -> bool {
    matches!(event, RunEvent::Stdout(_) | RunEvent::Stderr(_))
}

impl EventQueue {
    fn new() -> Self {
        EventQueue {
            state: Mutex::new(QueueState::default()),
            ready: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, QueueState> {
        match self.state.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    /// Enqueues a line event, or drops it when [`MAX_PENDING_EVENTS`] lines are pending.
    fn push_line(&self, event: RunEvent) {
        let mut st = self.lock();
        if st.pending_lines >= MAX_PENDING_EVENTS {
            st.dropped = st.dropped.saturating_add(1);
            return;
        }
        st.events.push_back(event);
        st.pending_lines += 1;
        drop(st);
        self.ready.notify_one();
    }

    /// Enqueues `Started` / `Finished`, which are never dropped.
    fn push_control(&self, event: RunEvent) {
        let mut st = self.lock();
        st.events.push_back(event);
        drop(st);
        self.ready.notify_all();
    }

    /// Marks the end of the stream so a blocked reader returns instead of hanging.
    fn close(&self) {
        let mut st = self.lock();
        st.closed = true;
        drop(st);
        self.ready.notify_all();
    }

    fn dropped(&self) -> u64 {
        self.lock().dropped
    }

    fn try_pop(&self) -> Option<RunEvent> {
        self.lock().pop_front()
    }

    fn pop_timeout(&self, timeout: Duration) -> Option<RunEvent> {
        let started = Instant::now();
        let mut st = self.lock();
        loop {
            if let Some(e) = st.pop_front() {
                return Some(e);
            }
            let elapsed = started.elapsed();
            if elapsed >= timeout || st.closed {
                return None;
            }
            st = match self.ready.wait_timeout(st, timeout - elapsed) {
                Ok((g, _)) => g,
                Err(p) => p.into_inner().0,
            };
        }
    }

    fn pop_blocking(&self) -> Option<RunEvent> {
        let mut st = self.lock();
        loop {
            if let Some(e) = st.pop_front() {
                return Some(e);
            }
            if st.closed {
                return None;
            }
            st = match self.ready.wait(st) {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
        }
    }
}

/// Reads `reader` to its end and calls `on_line` for every line (without `\n` / `\r\n`).
///
/// Bytes that are not valid UTF-8 are replaced by U+FFFD instead of ending the read:
/// if the reader stopped early the pipe would fill up and the child would block forever.
fn drain_lines(reader: impl Read, mut on_line: impl FnMut(String)) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::with_capacity(MAX_LINE_BYTES);
    let mut truncated = false;
    loop {
        let (consumed, line_ended, eof) = match reader.fill_buf() {
            // End of stream: emit a final unterminated line if one was in progress.
            Ok([]) => (0, false, true),
            Ok(chunk) => {
                let line_end = chunk.iter().position(|byte| *byte == b'\n');
                let consumed = line_end.map_or(chunk.len(), |index| index + 1);
                let content_len = line_end.unwrap_or(consumed);
                let available = MAX_LINE_BYTES.saturating_sub(buf.len());
                let retained = content_len.min(available);
                buf.extend_from_slice(&chunk[..retained]);
                truncated |= retained < content_len;
                (consumed, line_end.is_some(), false)
            }
            // A broken pipe ends this stream, as before.
            Err(_) => break,
        };
        if eof {
            if !buf.is_empty() || truncated {
                on_line(finish_line(&buf, false, truncated));
            }
            break;
        }
        reader.consume(consumed);
        if line_ended {
            on_line(finish_line(&buf, true, truncated));
            buf.clear();
            truncated = false;
        }
    }
}

fn finish_line(bytes: &[u8], line_ended: bool, truncated: bool) -> String {
    let mut bytes = bytes;
    if line_ended && !truncated && bytes.last() == Some(&b'\r') {
        bytes = &bytes[..bytes.len() - 1];
    }
    let mut line = String::from_utf8_lossy(bytes).into_owned();
    let needs_marker = truncated || line.len() > MAX_LINE_BYTES;
    if needs_marker {
        let content_limit = MAX_LINE_BYTES - LINE_TRUNCATION_MARKER.len();
        let mut end = content_limit.min(line.len());
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        line.truncate(end);
        line.push_str(LINE_TRUNCATION_MARKER);
    }
    line
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
        // A previous run may have left its results here. The child is not guaranteed to
        // overwrite them, and a failed run must never be read as a successful one (#207).
        for name in [crate::results::RESULTS_CSV, crate::results::RESULTS_META] {
            match std::fs::remove_file(out_dir.join(name)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(RunnerError::Io(e)),
            }
        }
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
        let events = Arc::new(EventQueue::new());
        events.push_control(RunEvent::Started);
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let out_events = events.clone();
        let out_thread = std::thread::spawn(move || {
            let mut all = RetainedLines::default();
            if let Some(s) = stdout {
                drain_lines(s, |line| {
                    out_events.push_line(RunEvent::Stdout(line.clone()));
                    all.push(line);
                });
            }
            all.into_vec()
        });
        let err_events = events.clone();
        let err_thread = std::thread::spawn(move || {
            let mut all = RetainedLines::default();
            if let Some(s) = stderr {
                drain_lines(s, |line| {
                    err_events.push_line(RunEvent::Stderr(line.clone()));
                    all.push(line);
                });
            }
            all.into_vec()
        });
        let child = Arc::new(Mutex::new(Some(child)));
        let stop = Arc::new(AtomicBool::new(false));
        let (c2, s2) = (child.clone(), stop.clone());
        let finished_events = events.clone();
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
            finished_events.push_control(RunEvent::Finished(RunOutcome {
                status,
                stdout: out,
                stderr: err,
                elapsed: started.elapsed(),
            }));
            finished_events.close();
        });
        Ok(RunHandle {
            child,
            events,
            finished: false,
            stop_requested: stop,
        })
    }

    /// Non-blocking: up to [`MAX_PENDING_EVENTS`] pending events.
    ///
    /// The drain is bounded so a caller that polls in a loop cannot be kept inside one
    /// call by a child that prints as fast as the loop removes events.
    pub fn poll_events(&mut self) -> Vec<RunEvent> {
        let mut v = vec![];
        while v.len() < MAX_PENDING_EVENTS {
            let Some(e) = self.events.try_pop() else {
                break;
            };
            if matches!(e, RunEvent::Finished(_)) {
                self.finished = true;
            }
            v.push(e);
        }
        v
    }

    /// Blocks until the next event (or timeout).
    pub fn wait_event(&mut self, timeout: Duration) -> Option<RunEvent> {
        let e = self.events.pop_timeout(timeout)?;
        if matches!(e, RunEvent::Finished(_)) {
            self.finished = true;
        }
        Some(e)
    }

    /// Line events dropped because the caller polled slower than the child printed.
    ///
    /// Only the live preview is affected: the outcome returned by `Finished` always
    /// carries the newest [`MAX_RETAINED_LINES`] lines of both streams.
    pub fn dropped_events(&self) -> u64 {
        self.events.dropped()
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
        match h.events.pop_blocking() {
            Some(RunEvent::Finished(o)) => return Ok(o),
            Some(_) => {}
            None => {
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
    fn drain_lines_bounds_long_lines_and_keeps_reading() {
        let mut input = vec![b'x'; MAX_LINE_BYTES * 8];
        input.push(b'\n');
        input.extend_from_slice(b"after\n");
        let lines = lines_of(&input);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].len() <= MAX_LINE_BYTES);
        assert!(lines[0].ends_with(LINE_TRUNCATION_MARKER));
        assert_eq!(lines[1], "after");
    }

    #[test]
    fn drain_lines_bounds_an_unterminated_long_line() {
        let input = vec![b'x'; MAX_LINE_BYTES * 8];
        let lines = lines_of(&input);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() <= MAX_LINE_BYTES);
        assert!(lines[0].ends_with(LINE_TRUNCATION_MARKER));
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

    fn outcome() -> RunOutcome {
        RunOutcome {
            status: RunStatus::Succeeded,
            stdout: vec![],
            stderr: vec![],
            elapsed: Duration::ZERO,
        }
    }

    #[test]
    fn event_queue_drops_line_events_past_the_bound() {
        let q = EventQueue::new();
        q.push_control(RunEvent::Started);
        for i in 0..MAX_PENDING_EVENTS + 500 {
            q.push_line(RunEvent::Stdout(i.to_string()));
        }
        // `Started` does not use a line slot, so exactly the 500 excess lines are dropped.
        assert_eq!(q.dropped(), 500);
        let mut seen = vec![];
        while let Some(e) = q.try_pop() {
            seen.push(e);
        }
        assert_eq!(seen.len(), MAX_PENDING_EVENTS + 1);
        assert_eq!(seen[0], RunEvent::Started);
        assert_eq!(seen[1], RunEvent::Stdout("0".into()));
        // Draining the lines frees the slots again.
        for i in 0..MAX_PENDING_EVENTS {
            q.push_line(RunEvent::Stdout(i.to_string()));
        }
        assert_eq!(q.dropped(), 500);
    }

    #[test]
    fn event_queue_keeps_finished_even_when_full() {
        let q = EventQueue::new();
        for i in 0..MAX_PENDING_EVENTS * 2 {
            q.push_line(RunEvent::Stderr(i.to_string()));
        }
        assert_eq!(q.dropped(), MAX_PENDING_EVENTS as u64);
        q.push_control(RunEvent::Finished(outcome()));
        let mut seen = vec![];
        while let Some(e) = q.try_pop() {
            seen.push(e);
        }
        assert_eq!(seen.len(), MAX_PENDING_EVENTS + 1);
        assert_eq!(seen.last(), Some(&RunEvent::Finished(outcome())));
    }

    #[test]
    fn closed_empty_queue_returns_instead_of_blocking() {
        let q = EventQueue::new();
        q.close();
        assert!(q.pop_blocking().is_none());
        assert!(q.pop_timeout(Duration::from_millis(10)).is_none());
    }

    #[test]
    fn event_queue_wakes_a_waiter_on_push() {
        let q = Arc::new(EventQueue::new());
        let producer = q.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            producer.push_line(RunEvent::Stdout("late".into()));
            producer.close();
        });
        assert_eq!(
            q.pop_blocking(),
            Some(RunEvent::Stdout("late".into())),
            "pop_blocking must wake up when a line arrives"
        );
        assert!(q.pop_timeout(Duration::from_secs(1)).is_none());
        t.join().expect("producer thread");
    }
}
