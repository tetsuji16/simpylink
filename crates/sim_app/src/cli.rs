//! Command line interface (DES-01 §5).
use clap::{Parser, Subcommand};
use sim_codegen_py::{generate, CodegenOptions};
use sim_core::diag::Diagnostic;
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
    },
    /// Check the Python environment
    CheckPython {
        #[arg(long)]
        python: Option<PathBuf>,
    },
}

pub const EXIT_OK: i32 = 0;
pub const EXIT_RUN_FAILED: i32 = 1;
pub const EXIT_MODEL_ERROR: i32 = 3;
pub const EXIT_IO_ERROR: i32 = 4;
pub const EXIT_PYTHON_ENV: i32 = 5;

/// Prints diagnostics to stderr.
pub fn print_diagnostics(diags: &[Diagnostic], model: &Model) {
    for d in diags {
        let mut line = format!("{}[{}]: {}", sev(d), d.code, d.message);
        if let Some(b) = d.location.block {
            let name = find_name(model, b).unwrap_or_default();
            line.push_str(&format!(" (block {b} \"{name}\")"));
        }
        eprintln!("{line}");
    }
}

fn sev(d: &Diagnostic) -> &'static str {
    match d.severity {
        sim_core::diag::Severity::Error => "error",
        sim_core::diag::Severity::Warning => "warning",
        sim_core::diag::Severity::Info => "info",
    }
}

fn find_name(m: &Model, sid: sim_core::ids::BlockId) -> Option<String> {
    fn rec(s: &sim_core::model::System, sid: sim_core::ids::BlockId) -> Option<String> {
        for b in &s.blocks {
            if b.sid == sid {
                return Some(b.name.clone());
            }
            if let Some(c) = &b.system {
                if let Some(n) = rec(c, sid) {
                    return Some(n);
                }
            }
        }
        None
    }
    rec(&m.root, sid)
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

/// `simpylink run`.
pub fn run_run(model: &Path, out_dir: &Path, python: Option<&Path>, keep_script: bool) -> i32 {
    let (m, code) = match build(model) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let cfg = RunnerConfig {
        python: python.map(Path::to_path_buf),
        timeout: None,
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
    let outcome = match run_blocking(&env, &script, out_dir, None) {
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
        let fin: Vec<f64> = c.values.iter().copied().filter(|v| v.is_finite()).collect();
        let min = fin.iter().copied().fold(f64::INFINITY, f64::min);
        let max = fin.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        println!(
            "  {}: min {}, max {}, final {}",
            c.name,
            min,
            max,
            c.values.last().copied().unwrap_or(f64::NAN)
        );
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
        } => run_run(model, out_dir, python.as_deref(), *keep_script),
        CliCommand::CheckPython { python } => run_check_python(python.as_deref()),
    }
}
