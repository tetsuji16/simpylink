//! Command line interface (DES-01 §5).
#[path = "report.rs"]
mod report;
use clap::{Parser, Subcommand};
use report::print_diagnostics;
use sim_codegen_py::{generate, CodegenOptions};
use sim_core::model::Model;
use sim_runner::{
    check_environment, discover_python, load_results, run_blocking, RunStatus, RunnerConfig,
};
use std::path::{Path, PathBuf};

/// Top-level CLI.
#[derive(Parser, Debug)]
#[command(
    name = "simpylink",
    version,
    about = "Open-source block diagram simulator"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

/// Subcommands.
#[derive(Subcommand, Debug, PartialEq)]
pub enum CliCommand {
    /// Open the GUI
    Gui { file: Option<PathBuf> },
    /// Generate Python code from a model JSON
    Codegen {
        model: PathBuf,
        #[arg(short = 'o', long = "output")]
        output: PathBuf,
    },
    /// Generate, run and check a model
    Run {
        model: PathBuf,
        #[arg(long = "out-dir")]
        out_dir: PathBuf,
        #[arg(long)]
        python: Option<PathBuf>,
        #[arg(long = "keep-script")]
        keep_script: bool,
        /// Give up after this many seconds (default: no limit)
        #[arg(long, value_parser = parse_timeout)]
        timeout: Option<f64>,
    },
    /// Check the Python environment
    CheckPython {
        #[arg(long)]
        python: Option<PathBuf>,
    },
}

/// Exit code of a successful command.
pub const EXIT_OK: i32 = 0;
/// Exit code of a failed simulation run.
pub const EXIT_RUN_FAILED: i32 = 1;
/// Exit code of a model that does not compile.
pub const EXIT_MODEL_ERROR: i32 = 3;
/// Exit code of an I/O error.
pub const EXIT_IO_ERROR: i32 = 4;
/// Exit code of a broken Python environment.
pub const EXIT_PYTHON_ENV: i32 = 5;

/// Parses `--timeout`: a positive, finite number of seconds that fits a `Duration`.
fn parse_timeout(raw: &str) -> Result<f64, String> {
    let bad = || format!("timeout must be a positive number of seconds, got '{raw}'");
    let secs = raw.trim().parse::<f64>().map_err(|_| bad())?;
    timeout_duration(secs).map_err(|_| bad())?;
    Ok(secs)
}

fn timeout_duration(secs: f64) -> Result<std::time::Duration, ()> {
    if !secs.is_finite() || secs <= 0.0 {
        return Err(());
    }
    std::time::Duration::try_from_secs_f64(secs).map_err(|_| ())
}

fn build(model_path: &Path) -> Result<(Model, sim_codegen_py::GeneratedCode), i32> {
    let model = sim_core::json::load_from_path(model_path).map_err(|e| {
        eprintln!("error[E_IO]: {e}");
        EXIT_IO_ERROR
    })?;
    let compiled = match sim_ir::compile(&model) {
        Ok(c) => c,
        Err(f) => {
            print_diagnostics(&f.diagnostics, &model);
            return Err(EXIT_MODEL_ERROR);
        }
    };
    print_diagnostics(&compiled.diagnostics, &model);
    let code = generate(&compiled.ir, &CodegenOptions::default()).map_err(|e| {
        eprintln!("error[E_CODEGEN]: {e}");
        EXIT_MODEL_ERROR
    })?;
    Ok((model, code))
}

/// `simpylink codegen`.
pub fn run_codegen(model: &Path, output: &Path) -> i32 {
    let (_, code) = match build(model) {
        Ok(x) => x,
        Err(c) => return c,
    };
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("error[E_IO]: {e}");
            return EXIT_IO_ERROR;
        }
    }
    if let Err(e) = std::fs::write(output, code.source) {
        eprintln!("error[E_IO]: {e}");
        return EXIT_IO_ERROR;
    }
    println!("wrote {}", output.display());
    EXIT_OK
}

/// Formats one channel summary line.
///
/// A channel whose samples are all NaN / Inf has no range to report, so the line says
/// `no finite samples` instead of `min inf max -inf` (#207).
fn channel_summary(c: &sim_runner::Channel) -> String {
    let fin: Vec<f64> = c.values.iter().copied().filter(|v| v.is_finite()).collect();
    match (
        fin.iter().copied().reduce(f64::min),
        fin.iter().copied().reduce(f64::max),
    ) {
        (Some(min), Some(max)) => format!(
            "  {}: min {}, max {}, final {}",
            c.name,
            min,
            max,
            c.values.last().copied().unwrap_or(f64::NAN)
        ),
        _ => format!("  {}: no finite samples", c.name),
    }
}

/// `simpylink run`.
pub fn run_run(
    model: &Path,
    out_dir: &Path,
    python: Option<&Path>,
    keep_script: bool,
    timeout: Option<std::time::Duration>,
) -> i32 {
    let (m, code) = match build(model) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let cfg = RunnerConfig {
        python: python.map(Path::to_path_buf),
        timeout,
    };
    let env = match discover_python(&cfg).and_then(|e| check_environment(&e).map(|_| e)) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error[E_PYTHON]: {e}");
            return EXIT_PYTHON_ENV;
        }
    };
    if let Err(e) = std::fs::create_dir_all(out_dir) {
        eprintln!("error[E_IO]: {e}");
        return EXIT_IO_ERROR;
    }
    let script = out_dir.join(&code.file_name);
    if let Err(e) = std::fs::write(&script, &code.source) {
        eprintln!("error[E_IO]: {e}");
        return EXIT_IO_ERROR;
    }
    let outcome = match run_blocking(&env, &script, out_dir, timeout) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error[E_RUN]: {e}");
            return EXIT_RUN_FAILED;
        }
    };
    for l in outcome
        .stdout
        .iter()
        .filter(|l| !l.starts_with("SIMPYLINK:"))
    {
        println!("{l}");
    }
    for l in &outcome.stderr {
        eprintln!("{l}");
    }
    if let RunStatus::Failed { message, .. } = &outcome.status {
        eprintln!("error[E_RUN]: {message}");
        return EXIT_RUN_FAILED;
    }
    if let RunStatus::TimedOut = outcome.status {
        match timeout {
            Some(t) => eprintln!("simulation timed out after {} s", t.as_secs_f64()),
            None => eprintln!("simulation timed out"),
        }
        return EXIT_RUN_FAILED;
    }
    if outcome.status != RunStatus::Succeeded {
        eprintln!("error[E_RUN]: {:?}", outcome.status);
        return EXIT_RUN_FAILED;
    }
    let r = match load_results(out_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error[E_RESULTS]: {e}");
            return EXIT_RUN_FAILED;
        }
    };
    println!(
        "simulation finished: {} samples, t = {} .. {}",
        r.time.len(),
        m.config.start_time,
        m.config.stop_time
    );
    for c in &r.channels {
        println!("{}", channel_summary(c));
    }
    println!("results: {}", out_dir.join("results.csv").display());
    if !keep_script {
        let _removed = std::fs::remove_file(&script);
    }
    EXIT_OK
}

/// `simpylink check-python`.
pub fn run_check_python(python: Option<&Path>) -> i32 {
    let cfg = RunnerConfig {
        python: python.map(Path::to_path_buf),
        timeout: None,
    };
    match discover_python(&cfg).and_then(|e| check_environment(&e).map(|r| (e, r))) {
        Ok((e, r)) => {
            println!(
                "python {} ({} {})",
                r.python_version,
                e.program.display(),
                e.prefix_args.join(" ")
            );
            println!("numpy {}", r.numpy_version.unwrap_or_default());
            println!("scipy {}", r.scipy_version.unwrap_or_default());
            EXIT_OK
        }
        Err(e) => {
            eprintln!("error[E_PYTHON]: {e}");
            EXIT_PYTHON_ENV
        }
    }
}

/// Runs a non-GUI command.
pub fn run_cli(cmd: &CliCommand) -> i32 {
    match cmd {
        CliCommand::Gui { .. } => EXIT_OK,
        CliCommand::Codegen { model, output } => run_codegen(model, output),
        CliCommand::Run {
            model,
            out_dir,
            python,
            keep_script,
            timeout,
        } => {
            let timeout = match timeout.map(|seconds| timeout_duration(seconds)) {
                Some(Ok(duration)) => Some(duration),
                Some(Err(())) => {
                    eprintln!("error: timeout must be a positive number of seconds");
                    return EXIT_RUN_FAILED;
                }
                None => None,
            };
            run_run(model, out_dir, python.as_deref(), *keep_script, timeout)
        }
        CliCommand::CheckPython { python } => run_check_python(python.as_deref()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(values: Vec<f64>) -> sim_runner::Channel {
        sim_runner::Channel {
            name: "Scope/1".into(),
            block_sid: 3,
            port: 1,
            values,
        }
    }

    #[test]
    fn channel_summary_reports_the_finite_range() {
        assert_eq!(
            channel_summary(&channel(vec![0.0, 6.0, 3.0])),
            "  Scope/1: min 0, max 6, final 3"
        );
    }

    #[test]
    fn channel_summary_skips_non_finite_samples() {
        assert_eq!(
            channel_summary(&channel(vec![f64::NAN, 2.0, f64::INFINITY])),
            "  Scope/1: min 2, max 2, final inf"
        );
    }

    #[test]
    fn channel_summary_says_so_when_no_sample_is_finite() {
        assert_eq!(
            channel_summary(&channel(vec![f64::NAN, f64::INFINITY])),
            "  Scope/1: no finite samples"
        );
        assert_eq!(
            channel_summary(&channel(vec![])),
            "  Scope/1: no finite samples"
        );
    }

    #[test]
    fn timeout_parser_accepts_only_positive_finite_numbers() {
        assert_eq!(parse_timeout("1"), Ok(1.0));
        assert_eq!(parse_timeout("0.5"), Ok(0.5));
        assert!(parse_timeout("0").is_err());
        assert!(parse_timeout("-1").is_err());
        assert!(parse_timeout("abc").is_err());
        assert!(parse_timeout("inf").is_err());
        assert!(parse_timeout("nan").is_err());
        // Finite and positive, but too large for a `Duration`.
        assert!(parse_timeout("1e300").is_err());
    }

    #[test]
    fn programmatic_timeout_conversion_rejects_invalid_values_without_panicking() {
        assert_eq!(
            timeout_duration(1.5),
            Ok(std::time::Duration::from_millis(1500))
        );
        for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e300] {
            assert!(timeout_duration(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn run_cli_rejects_invalid_programmatic_timeouts_without_panicking() {
        for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e300] {
            let command = CliCommand::Run {
                model: PathBuf::from("unused.json"),
                out_dir: PathBuf::from("unused-results"),
                python: None,
                keep_script: false,
                timeout: Some(invalid),
            };
            assert_eq!(run_cli(&command), EXIT_RUN_FAILED, "{invalid}");
        }
    }
}
