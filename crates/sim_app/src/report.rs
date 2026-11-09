//! Diagnostic output for the CLI (DES-01 section 5, M2-07 section E/F).
//!
//! One diagnostic per line, on stderr. When the diagnostic has a block the
//! block name is appended as ` (block <sid> "<name>")` so that the message can
//! be located in the diagram.
use sim_core::diag::{Diagnostic, Severity};
use sim_core::ids::BlockId;
use sim_core::model::{Model, System};

/// Prints diagnostics to stderr, one per line.
pub fn print_diagnostics(diags: &[Diagnostic], model: &Model) {
    for line in format_lines(diags, model) {
        eprintln!("{line}");
    }
}

fn format_lines(diags: &[Diagnostic], model: &Model) -> Vec<String> {
    diags
        .iter()
        .map(|d| {
            let mut line = format!("{}[{}]: {}", severity(d.severity), d.code, d.message);
            if let Some(b) = d.location.block {
                let name = find_name(&model.root, b).unwrap_or_default();
                line.push_str(&format!(" (block {b} \"{name}\")"));
            }
            line
        })
        .collect()
}

fn severity(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn find_name(sys: &System, sid: BlockId) -> Option<String> {
    for b in &sys.blocks {
        if b.sid == sid {
            return Some(b.name.clone());
        }
        if let Some(c) = &b.system {
            if let Some(n) = find_name(c, sid) {
                return Some(n);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::diag::Location;
    use sim_core::json::from_json_str;
    use sim_core::path::SystemPath;

    fn fixture(name: &str) -> Model {
        let p = format!(
            "{}/../../tests/fixtures/models/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        from_json_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    #[test]
    fn formats_severity_code_and_block_name() {
        let m = fixture("step_gain_scope");
        let diags = vec![
            Diagnostic::error(
                "E_PARAM_EVAL",
                "parameter 'Gain' of 'Gain': unknown identifier 'K' (at byte 0)",
                Location::block(&SystemPath::root(), BlockId(2)),
            ),
            Diagnostic::warning(
                "W_UNCONNECTED_INPUT",
                "input port 1 of 'Scope' is not connected; it will read 0",
                Location::block(&SystemPath::root(), BlockId(3)),
            ),
            Diagnostic::info("I_NOTE", "no block location", Location::default()),
        ];
        assert_eq!(
            format_lines(&diags, &m),
            vec![
                "error[E_PARAM_EVAL]: parameter 'Gain' of 'Gain': unknown identifier 'K' (at byte 0) (block 2 \"Gain\")",
                "warning[W_UNCONNECTED_INPUT]: input port 1 of 'Scope' is not connected; it will read 0 (block 3 \"Scope\")",
                "info[I_NOTE]: no block location",
            ]
        );
    }

    #[test]
    fn unknown_block_prints_empty_name() {
        let m = fixture("step_gain_scope");
        let diags = vec![Diagnostic::error(
            "E_INTERNAL",
            "boom",
            Location::block(&SystemPath::root(), BlockId(99)),
        )];
        assert_eq!(
            format_lines(&diags, &m),
            vec!["error[E_INTERNAL]: boom (block 99 \"\")"]
        );
    }

    #[test]
    fn finds_blocks_inside_subsystems() {
        let m = fixture("subsystem_gain");
        let inner = m.root.blocks[1]
            .system
            .as_ref()
            .and_then(|s| s.blocks.iter().find(|b| b.block_type == "Gain"))
            .map(|b| b.sid)
            .expect("subsystem_gain has an inner Gain block");
        assert_eq!(find_name(&m.root, inner), Some("Gain".to_string()));
        assert_eq!(find_name(&m.root, BlockId(999)), None);
    }
}
