//! Python discovery and environment check.
use crate::types::*;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Hides console windows on Windows.
pub(crate) fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let _ = cmd;
}

/// "3.12.1" → (3, 12, 1).
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let p: Vec<&str> = s.trim().split('.').collect();
    if p.len() != 3 {
        return None;
    }
    Some((
        p[0].parse().ok()?,
        p[1].parse().ok()?,
        p[2].trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()?,
    ))
}

/// Candidate interpreters in search order.
pub fn candidates(
    cfg: &RunnerConfig,
    env_var: Option<String>,
    windows: bool,
) -> Vec<(PathBuf, Vec<String>)> {
    if let Some(p) = &cfg.python {
        return vec![(p.clone(), vec![])];
    }
    if let Some(e) = env_var.filter(|e| !e.trim().is_empty()) {
        return vec![(PathBuf::from(e), vec![])];
    }
    if windows {
        vec![
            ("py".into(), vec!["-3".into()]),
            ("python".into(), vec![]),
            ("python3".into(), vec![]),
        ]
    } else {
        vec![("python3".into(), vec![]), ("python".into(), vec![])]
    }
}

fn run_capture(
    program: &PathBuf,
    args: &[String],
    timeout: Duration,
) -> Option<(i32, String, String)> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    no_window(&mut cmd);
    let child = cmd.spawn().ok()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let r = child.wait_with_output();
        let _sent = tx.send(r);
    });
    let out = rx.recv_timeout(timeout).ok()?.ok()?;
    Some((
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into(),
        String::from_utf8_lossy(&out.stderr).into(),
    ))
}

/// Finds Python 3.10+.
pub fn discover_python(cfg: &RunnerConfig) -> Result<PythonEnv, RunnerError> {
    let cands = candidates(cfg, std::env::var("SIMPYLINK_PYTHON").ok(), cfg!(windows));
    let mut tried = vec![];
    for (prog, prefix) in cands {
        let label = std::iter::once(prog.display().to_string())
            .chain(prefix.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        tried.push(label);
        let mut args = prefix.clone();
        args.extend([
            "-c".to_string(),
            "import sys; print('%d.%d.%d' % sys.version_info[:3])".to_string(),
        ]);
        if let Some((0, out, _)) = run_capture(&prog, &args, Duration::from_secs(10)) {
            if let Some(v) = parse_version(out.trim()) {
                if v >= (3, 10, 0) {
                    return Ok(PythonEnv {
                        program: prog,
                        prefix_args: prefix,
                        version: out.trim().to_string(),
                    });
                }
            }
        }
    }
    Err(RunnerError::PythonNotFound { tried })
}

/// Checks numpy / scipy.
pub fn check_environment(env: &PythonEnv) -> Result<EnvReport, RunnerError> {
    let mut args = env.prefix_args.clone();
    args.extend([
        "-c".to_string(),
        "import numpy, scipy; print(numpy.__version__); print(scipy.__version__)".to_string(),
    ]);
    let (code, out, err) = run_capture(&env.program, &args, Duration::from_secs(60))
        .ok_or_else(|| RunnerError::Spawn("python did not respond".into()))?;
    let hint = "python -m pip install numpy scipy".to_string();
    if code != 0 {
        for name in ["numpy", "scipy"] {
            if err.contains(name) {
                return Err(RunnerError::MissingPackage {
                    name: name.into(),
                    hint,
                });
            }
        }
        return Err(RunnerError::Spawn(err));
    }
    let mut lines = out.lines().map(|s| s.trim().to_string());
    Ok(EnvReport {
        python_version: env.version.clone(),
        numpy_version: lines.next(),
        scipy_version: lines.next(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pure() {
        assert_eq!(parse_version("3.12.1"), Some((3, 12, 1)));
        assert_eq!(parse_version("3.9.18"), Some((3, 9, 18)));
        assert_eq!(parse_version("abc"), None);
        assert_eq!(parse_version("3.12"), None);
        let d = RunnerConfig::default();
        assert_eq!(
            candidates(&d, None, true)[0],
            (PathBuf::from("py"), vec!["-3".to_string()])
        );
        assert_eq!(candidates(&d, None, true).len(), 3);
        let c = RunnerConfig {
            python: Some("/opt/py".into()),
            timeout: None,
        };
        assert_eq!(
            candidates(&c, Some("x".into()), false),
            vec![(PathBuf::from("/opt/py"), vec![])]
        );
        assert_eq!(
            candidates(&d, Some("/usr/bin/python3.12".into()), false),
            vec![(PathBuf::from("/usr/bin/python3.12"), vec![])]
        );
    }
}
