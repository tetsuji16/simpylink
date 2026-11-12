//! Python identifier rules (DES-03 §8).
use sim_core::ids::BlockId;

/// Converts a block name into a snake_case Python identifier fragment.
pub fn sanitize_identifier(name: &str) -> String {
    let s: Vec<char> = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let mut out = String::new();
    for (i, c) in s.iter().enumerate() {
        if i > 0
            && c.is_ascii_uppercase()
            && (s[i - 1].is_ascii_lowercase() || s[i - 1].is_ascii_digit())
        {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    let mut collapsed = String::new();
    for c in out.chars() {
        if c == '_' && collapsed.ends_with('_') {
            continue;
        }
        collapsed.push(c);
    }
    let mut r = collapsed.trim_matches('_').to_string();
    if r.is_empty() {
        r = "block".into();
    }
    if r.starts_with(|c: char| c.is_ascii_digit()) {
        r = format!("b_{r}");
    }
    r
}

/// "{sanitize(name)}_{sid}".
pub fn var_base(name: &str, sid: BlockId) -> String {
    format!("{}_{}", sanitize_identifier(name), sid.0)
}

/// Signal variable name of an output port.
pub fn signal_var_name(var_base: &str, port: u32, n_outputs: u32) -> String {
    if n_outputs == 1 {
        var_base.to_string()
    } else {
        format!("{var_base}_y{port}")
    }
}

/// Log channel name ("Scope/1" or "Out1").
///
/// Control characters and the CSV separators `,` / `"` are replaced by `_` (#203).
pub fn log_name(block_name: &str, port: Option<u32>) -> String {
    let clean: String = block_name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ',' | '"') {
                '_'
            } else {
                c
            }
        })
        .collect();
    match port {
        Some(p) => format!("{clean}/{p}"),
        None => clean,
    }
}

/// Channel name of a block inside `parents` (subsystem names, outermost first).
///
/// Blocks at the root keep their plain name so existing result files stay readable;
/// blocks inside a subsystem are qualified as `Parent/Child`. Two subsystems may hold
/// a block of the same name, so the qualification is what keeps the channels distinct
/// (#203).
pub fn log_name_at(parents: &[String], block_name: &str, port: Option<u32>) -> String {
    if parents.is_empty() {
        return log_name(block_name, port);
    }
    let mut full = parents.join("/");
    full.push('/');
    full.push_str(block_name);
    log_name(&full, port)
}

/// Script file name for a model.
pub fn script_file_name(model_name: &str) -> String {
    let trimmed = model_name.trim();
    let s = if trimmed.chars().any(|c| c.is_ascii_alphanumeric()) {
        sanitize_identifier(trimmed)
    } else {
        "model".into()
    };
    format!("{s}.py")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        for (i, o) in [
            ("Gain", "gain"),
            ("Unit Delay", "unit_delay"),
            ("TransferFcn1", "transfer_fcn1"),
            ("Discrete-Time Integrator", "discrete_time_integrator"),
            ("増幅", "block"),
            ("1abc", "b_1abc"),
            ("x2Y", "x2_y"),
            ("  My__Block  ", "my_block"),
            ("PIDController", "pidcontroller"),
        ] {
            assert_eq!(sanitize_identifier(i), o, "{i}");
        }
        assert_eq!(var_base("Gain", BlockId(2)), "gain_2");
        assert_eq!(signal_var_name("gain_2", 1, 1), "gain_2");
        assert_eq!(signal_var_name("demux_4", 2, 3), "demux_4_y2");
        assert_eq!(log_name("Scope", Some(1)), "Scope/1");
        assert_eq!(log_name("a,b\"c", None), "a_b_c");
        assert_eq!(log_name("a\tb\nc", None), "a_b_c");
        assert_eq!(
            log_name_at(&["A".to_string(), "Inner".to_string()], "Scope", Some(1)),
            "A/Inner/Scope/1"
        );
        assert_eq!(log_name_at(&[], "Out1", None), "Out1");
        assert_eq!(script_file_name(""), "model.py");
        assert_eq!(script_file_name("My Model"), "my_model.py");
        assert_eq!(script_file_name("step_gain_scope"), "step_gain_scope.py");
    }
}
