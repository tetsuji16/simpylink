//! Simulation session: compile → generate → run in background → load results.
use sim_codegen_py::{generate, CodegenOptions, GeneratedCode};
use sim_core::diag::{has_errors, Diagnostic};
use sim_core::model::Model;
use sim_runner::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Run phase.
pub enum SimPhase {
    Idle,
    Running {
        handle: RunHandle,
        out_dir: PathBuf,
        started: Instant,
    },
    Finished(SimSummary),
}

/// Result summary.
#[derive(Debug, Clone, PartialEq)]
pub struct SimSummary {
    pub status: RunStatus,
    pub elapsed: Duration,
    pub error: Option<String>,
    pub samples: usize,
}

/// Why a run could not start.
#[derive(Debug, Clone, PartialEq)]
pub enum StartError {
    Compile(Vec<Diagnostic>),
    Python(String),
    Io(String),
    AlreadyRunning,
}

/// Poll result.
#[derive(Debug, Clone, PartialEq)]
pub enum SimUpdate {
    None,
    Log(Vec<String>),
    Done {
        results: Option<SimResults>,
        summary: SimSummary,
        log: Vec<String>,
    },
}

/// Simulation session.
pub struct SimSession {
    pub phase: SimPhase,
    pub python: Option<PythonEnv>,
    pub runner_config: RunnerConfig,
    pub last_script: Option<PathBuf>,
    counter: u32,
}

impl Default for SimSession {
    fn default() -> Self {
        Self::new()
    }
}

/// temp_dir()/simpylink/<name>-<pid>-<counter>.
pub fn run_directory(model_name: &str, pid: u32, counter: u32) -> PathBuf {
    let stem = sim_ir::naming::script_file_name(model_name);
    let stem = stem.trim_end_matches(".py");
    std::env::temp_dir()
        .join("simpylink")
        .join(format!("{stem}-{pid}-{counter}"))
}

impl SimSession {
    /// New idle session.
    pub fn new() -> Self {
        SimSession {
            phase: SimPhase::Idle,
            python: None,
            runner_config: RunnerConfig::default(),
            last_script: None,
            counter: 0,
        }
    }
    /// Compile + generate (Ctrl+D).
    pub fn update_diagram(model: &Model) -> (Vec<Diagnostic>, Option<GeneratedCode>) {
        match sim_ir::compile(model) {
            Err(f) => (f.diagnostics, None),
            Ok(c) => match generate(&c.ir, &CodegenOptions::default()) {
                Ok(code) => (c.diagnostics, Some(code)),
                Err(e) => {
                    let mut d = c.diagnostics;
                    d.push(Diagnostic::error(
                        "E_CODEGEN",
                        e.to_string(),
                        Default::default(),
                    ));
                    (d, None)
                }
            },
        }
    }
    /// True while Python runs.
    pub fn is_running(&self) -> bool {
        matches!(self.phase, SimPhase::Running { .. })
    }
    /// Starts a run.
    pub fn start(&mut self, model: &Model) -> Result<Vec<Diagnostic>, StartError> {
        if self.is_running() {
            return Err(StartError::AlreadyRunning);
        }
        let (diags, code) = Self::update_diagram(model);
        let Some(code) = code.filter(|_| !has_errors(&diags)) else {
            return Err(StartError::Compile(diags));
        };
        if self.python.is_none() {
            let env = discover_python(&self.runner_config)
                .map_err(|e| StartError::Python(e.to_string()))?;
            check_environment(&env).map_err(|e| StartError::Python(e.to_string()))?;
            self.python = Some(env);
        }
        let Some(env) = self.python.clone() else {
            return Err(StartError::Python("no python".into()));
        };
        self.counter += 1;
        let dir = run_directory(&model.name, std::process::id(), self.counter);
        std::fs::create_dir_all(&dir).map_err(|e| StartError::Io(e.to_string()))?;
        let script = dir.join(&code.file_name);
        std::fs::write(&script, &code.source).map_err(|e| StartError::Io(e.to_string()))?;
        let handle =
            RunHandle::start(&env, &script, &dir).map_err(|e| StartError::Python(e.to_string()))?;
        self.last_script = Some(script);
        self.phase = SimPhase::Running {
            handle,
            out_dir: dir,
            started: Instant::now(),
        };
        Ok(diags)
    }
    /// Collects events; returns Done once.
    pub fn poll(&mut self) -> SimUpdate {
        let SimPhase::Running {
            handle, out_dir, ..
        } = &mut self.phase
        else {
            return SimUpdate::None;
        };
        let mut log = vec![];
        let mut done = None;
        for ev in handle.poll_events() {
            match ev {
                RunEvent::Stdout(l) if !l.starts_with("SIMPYLINK:") => log.push(l),
                RunEvent::Stderr(l) if !l.starts_with("SIMPYLINK:") => log.push(l),
                RunEvent::Finished(o) => done = Some(o),
                _ => {}
            }
        }
        let Some(o) = done else {
            return if log.is_empty() {
                SimUpdate::None
            } else {
                SimUpdate::Log(log)
            };
        };
        let (results, error) = match &o.status {
            RunStatus::Succeeded => match load_results(out_dir) {
                Ok(r) => (Some(r), None),
                Err(e) => (None, Some(e.to_string())),
            },
            RunStatus::Failed { message, .. } => (None, Some(message.clone())),
            RunStatus::Stopped => (None, None),
            RunStatus::TimedOut => (None, Some("timed out".into())),
        };
        let summary = SimSummary {
            status: o.status.clone(),
            elapsed: o.elapsed,
            error,
            samples: results.as_ref().map(|r| r.time.len()).unwrap_or(0),
        };
        self.phase = SimPhase::Finished(summary.clone());
        SimUpdate::Done {
            results,
            summary,
            log,
        }
    }
    /// Stops a running simulation.
    pub fn stop(&mut self) {
        if let SimPhase::Running { handle, .. } = &mut self.phase {
            handle.stop();
        }
    }
    /// Stops a running simulation and forgets it: no `SimUpdate::Done` follows.
    ///
    /// Used when the document is replaced (New / Open) or the window closes, so that
    /// results of the old model never reach the new one.
    pub fn abandon(&mut self) {
        self.stop();
        self.phase = SimPhase::Idle;
    }
}
