//! Runner types.
use std::path::PathBuf;
use std::time::Duration;

/// Runner configuration.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunnerConfig {
    pub python: Option<PathBuf>,
    pub timeout: Option<Duration>,
}

/// A Python interpreter.
#[derive(Debug, Clone, PartialEq)]
pub struct PythonEnv {
    pub program: PathBuf,
    pub prefix_args: Vec<String>,
    pub version: String,
}

/// Result of the environment check.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvReport {
    pub python_version: String,
    pub numpy_version: Option<String>,
    pub scipy_version: Option<String>,
}

/// Event from a running simulation.
#[derive(Debug, Clone, PartialEq)]
pub enum RunEvent {
    Started,
    Stdout(String),
    Stderr(String),
    Finished(RunOutcome),
}

/// Final status of a run.
#[derive(Debug, Clone, PartialEq)]
pub enum RunStatus {
    Succeeded,
    Failed {
        exit_code: Option<i32>,
        message: String,
    },
    Stopped,
    TimedOut,
}

/// Outcome of a run.
#[derive(Debug, Clone, PartialEq)]
pub struct RunOutcome {
    pub status: RunStatus,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    pub elapsed: Duration,
}

/// One logged channel.
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub name: String,
    pub block_sid: u32,
    pub port: u32,
    pub values: Vec<f64>,
}

/// Loaded simulation results.
#[derive(Debug, Clone, PartialEq)]
pub struct SimResults {
    pub model: String,
    pub time: Vec<f64>,
    pub channels: Vec<Channel>,
}

impl SimResults {
    /// Channel by name.
    pub fn channel(&self, name: &str) -> Option<&Channel> {
        self.channels.iter().find(|c| c.name == name)
    }
}

/// Runner errors.
#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("Python 3.10+ was not found (tried: {tried:?}). Set SIMPYLINK_PYTHON or choose Python in Settings.")]
    PythonNotFound { tried: Vec<String> },
    #[error("Python package '{name}' is missing. Install with: {hint}")]
    MissingPackage { name: String, hint: String },
    #[error("failed to start Python: {0}")]
    Spawn(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result-file errors.
#[derive(Debug, thiserror::Error)]
pub enum ResultsError {
    #[error("results file missing: {0}")]
    Missing(String),
    #[error("invalid results.meta.json: {0}")]
    Meta(String),
    #[error("invalid results.csv at line {line}: {message}")]
    Csv { line: usize, message: String },
    #[error("simulation reported error: {0}")]
    Reported(String),
}
