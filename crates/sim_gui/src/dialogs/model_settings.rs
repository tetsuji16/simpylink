//! Model Settings (Ctrl+E) dialog.
use super::{DialogButton, DialogOutcome};
use sim_core::edit::Command;
use sim_core::expr::parse_value;
use sim_core::model::{SimConfig, SolverKind};
use std::collections::BTreeMap;

/// Dialog state.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsDialogState {
    pub start: String,
    pub stop: String,
    pub solver: SolverKind,
    pub rel_tol: String,
    pub abs_tol: String,
    pub max_step: String,
    pub output_step: String,
    pub errors: BTreeMap<&'static str, String>,
    original: SimConfig,
}

fn num(s: &str) -> Option<f64> {
    parse_value(s).ok()?.as_scalar()
}

impl SettingsDialogState {
    /// Opens with the current configuration.
    pub fn open(cfg: &SimConfig) -> Self {
        SettingsDialogState {
            start: format!("{:?}", cfg.start_time),
            stop: format!("{:?}", cfg.stop_time),
            solver: cfg.solver,
            rel_tol: format!("{:?}", cfg.rel_tol),
            abs_tol: format!("{:?}", cfg.abs_tol),
            max_step: cfg
                .max_step
                .map(|v| format!("{v:?}"))
                .unwrap_or_else(|| "auto".into()),
            output_step: format!("{:?}", cfg.output_step),
            errors: BTreeMap::new(),
            original: cfg.clone(),
        }
    }
    /// Validates; Some(config) if valid.
    pub fn validate(&mut self) -> Option<SimConfig> {
        self.errors.clear();
        let mut get = |k: &'static str, s: &str| {
            let v = num(s);
            if v.map(|x| !x.is_finite()).unwrap_or(true) {
                self.errors
                    .insert(k, format!("{k} must be a finite number"));
            }
            v.unwrap_or(0.0)
        };
        let start = get("start", &self.start);
        let stop = get("stop", &self.stop);
        let rel = get("rel_tol", &self.rel_tol);
        let abs = get("abs_tol", &self.abs_tol);
        let step = get("output_step", &self.output_step);
        let max_step = if self.max_step.trim().eq_ignore_ascii_case("auto") {
            None
        } else {
            match num(&self.max_step) {
                Some(v) if v > 0.0 && v.is_finite() => Some(v),
                _ => {
                    self.errors
                        .insert("max_step", "max_step must be positive or auto".into());
                    None
                }
            }
        };
        if !self.errors.contains_key("stop") && stop <= start {
            self.errors
                .insert("stop", "stop_time must be greater than start_time".into());
        }
        if !self.errors.contains_key("output_step") && step <= 0.0 {
            self.errors
                .insert("output_step", "output_step must be positive".into());
        }
        if !self.errors.contains_key("rel_tol") && rel <= 0.0 {
            self.errors
                .insert("rel_tol", "rel_tol must be positive".into());
        }
        if !self.errors.contains_key("abs_tol") && abs <= 0.0 {
            self.errors
                .insert("abs_tol", "abs_tol must be positive".into());
        }
        if !self.errors.is_empty() {
            return None;
        }
        Some(SimConfig {
            start_time: start,
            stop_time: stop,
            solver: self.solver,
            rel_tol: rel,
            abs_tol: abs,
            max_step,
            output_step: step,
            fixed_step: self.original.fixed_step,
        })
    }
    /// Button press.
    pub fn press(&mut self, b: DialogButton) -> DialogOutcome {
        if b == DialogButton::Cancel {
            return DialogOutcome::Close;
        }
        let Some(cfg) = self.validate() else {
            return DialogOutcome::KeepOpen;
        };
        if cfg == self.original {
            return if b == DialogButton::Ok {
                DialogOutcome::Close
            } else {
                DialogOutcome::KeepOpen
            };
        }
        let cmd = Command::SetConfig {
            config: cfg.clone(),
        };
        if b == DialogButton::Ok {
            DialogOutcome::CloseAndExecute(cmd)
        } else {
            self.original = cfg;
            DialogOutcome::Execute(cmd)
        }
    }
}
