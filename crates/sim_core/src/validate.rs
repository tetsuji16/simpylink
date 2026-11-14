//! Structural model validation (deterministic order).
use crate::diag::{codes::*, Diagnostic, Location};
use crate::ids::{BlockId, LineId};
use crate::library::{port_counts, spec, validate_param, PortCounts};
use crate::model::{Block, Branch, Line, Model, PortRef, System};
use crate::path::SystemPath;
use std::collections::{BTreeMap, BTreeSet};

/// Validates the model and returns diagnostics in a deterministic order.
pub fn validate_model(model: &Model) -> Vec<Diagnostic> {
    let mut out = vec![];
    check_config(model, &mut out);
    let mut seen = BTreeSet::new();
    check_sids(&model.root, &SystemPath::root(), &mut seen, &mut out);
    let mut seen_lines = BTreeSet::new();
    check_system(&model.root, &SystemPath::root(), &mut seen_lines, &mut out);
    check_tags(model, &mut out);
    for loc in non_finite_geometry(model) {
        out.push(Diagnostic::error(
            E_NON_FINITE_GEOMETRY,
            "block position and line points must be finite",
            loc,
        ));
    }
    out
}

/// True when `s` holds a control character or a Unicode line/paragraph separator.
///
/// Such a name cannot be written to JSON, a `.m` script or generated Python without
/// escaping, so it is rejected at the model level (#198).
fn has_control_chars(s: &str) -> bool {
    s.chars()
        .any(|c| c.is_control() || c == '\u{2028}' || c == '\u{2029}')
}

fn finite_point(p: &crate::geom::Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

fn finite_branches(bs: &[Branch]) -> bool {
    bs.iter()
        .all(|b| b.points.iter().all(finite_point) && finite_branches(&b.branches))
}

fn finite_line(l: &Line) -> bool {
    l.points.iter().all(finite_point) && finite_branches(&l.branches)
}

/// Locations of blocks and lines whose geometry is NaN or ±Inf.
///
/// `serde_json` writes those as `null`, so a model saved with them cannot be loaded
/// again; they are reported here and refused by `save_to_path` (#198).
fn non_finite_geometry(model: &Model) -> Vec<Location> {
    fn walk(sys: &System, path: &SystemPath, out: &mut Vec<Location>) {
        for b in &sys.blocks {
            let r = &b.position;
            let finite = r.left.is_finite()
                && r.top.is_finite()
                && r.right.is_finite()
                && r.bottom.is_finite();
            if !finite {
                out.push(Location::block(path, b.sid));
            }
            if let Some(c) = &b.system {
                walk(c, &path.child(b.sid), out);
            }
        }
        for l in &sys.lines {
            if !finite_line(l) {
                out.push(Location::line(path, l.id));
            }
        }
    }
    let mut out = vec![];
    walk(&model.root, &SystemPath::root(), &mut out);
    out
}

/// True when any block position or line point is NaN or ±Inf.
pub fn has_non_finite_geometry(model: &Model) -> bool {
    !non_finite_geometry(model).is_empty()
}

fn check_tags(model: &Model, out: &mut Vec<Diagnostic>) {
    fn collect<'a>(
        sys: &'a System,
        path: &SystemPath,
        blocks: &mut Vec<(SystemPath, &'a crate::model::Block)>,
    ) {
        for b in &sys.blocks {
            if matches!(b.block_type.as_str(), "Goto" | "From" | "GotoTagVisibility") {
                blocks.push((path.clone(), b));
            }
            if let Some(child) = &b.system {
                collect(child, &path.child(b.sid), blocks);
            }
        }
    }
    let mut blocks = Vec::new();
    collect(&model.root, &SystemPath::root(), &mut blocks);
    for (path, b) in &blocks {
        if let Some(tag) = b.params.get("GotoTag") {
            if has_control_chars(tag) {
                out.push(Diagnostic::error(
                    E_INVALID_NAME,
                    "name must not contain control characters",
                    Location::block(path, b.sid),
                ));
            }
        }
    }
    let mut local = BTreeSet::new();
    let mut global = BTreeSet::new();
    let mut markers = BTreeSet::new();
    for (path, b) in &blocks {
        if b.block_type == "GotoTagVisibility" {
            let tag = b.params.get("GotoTag").map(String::as_str).unwrap_or("A");
            if !markers.insert((path.clone(), tag)) {
                out.push(Diagnostic::error(
                    E_DUPLICATE_GOTO_TAG,
                    format!("duplicate goto tag visibility for '{tag}'"),
                    Location::block(path, b.sid),
                ));
            }
        }
    }
    let scope_for = |path: &SystemPath, tag: &str| {
        let mut p = path.clone();
        loop {
            if markers.contains(&(p.clone(), tag)) {
                return Some(p);
            }
            let sid = p.0.pop()?;
            let parent = model.system(&p)?;
            if parent
                .block(sid)?
                .params
                .get("TreatAsAtomicUnit")
                .is_some_and(|v| v == "on")
            {
                return None;
            }
        }
    };
    let mut scoped = BTreeSet::new();
    for (path, b) in &blocks {
        if b.block_type != "Goto" {
            continue;
        }
        let tag = b.params.get("GotoTag").map(String::as_str).unwrap_or("A");
        let visibility = b
            .params
            .get("TagVisibility")
            .map(String::as_str)
            .unwrap_or("local");
        let duplicate = match visibility {
            "local" => !local.insert((path.clone(), tag)),
            "global" => !global.insert(tag),
            "scoped" => match scope_for(path, tag) {
                Some(scope) => !scoped.insert((scope, tag)),
                None => {
                    out.push(Diagnostic::error(
                        E_INVALID_PARAM,
                        format!("scoped goto tag '{tag}' has no visibility block"),
                        Location::block(path, b.sid),
                    ));
                    false
                }
            },
            _ => false,
        };
        if duplicate {
            out.push(Diagnostic::error(
                E_DUPLICATE_GOTO_TAG,
                format!("duplicate goto tag '{tag}'"),
                Location::block(path, b.sid),
            ));
        }
    }
    for (path, b) in &blocks {
        if b.block_type != "From" {
            continue;
        }
        let tag = b.params.get("GotoTag").map(String::as_str).unwrap_or("A");
        let scoped_match = scope_for(path, tag).is_some_and(|scope| scoped.contains(&(scope, tag)));
        if !local.contains(&(path.clone(), tag)) && !scoped_match && !global.contains(tag) {
            out.push(Diagnostic::error(
                E_UNMATCHED_FROM,
                format!("no visible goto for tag '{tag}'"),
                Location::block(path, b.sid),
            ));
        }
    }
}

fn check_config(m: &Model, out: &mut Vec<Diagnostic>) {
    let c = &m.config;
    let loc = Location::default;
    for (name, v) in [
        ("start_time", c.start_time),
        ("stop_time", c.stop_time),
        ("rel_tol", c.rel_tol),
        ("abs_tol", c.abs_tol),
        ("output_step", c.output_step),
    ] {
        if !v.is_finite() {
            out.push(Diagnostic::error(
                E_INVALID_CONFIG,
                format!("{name} must be finite"),
                loc(),
            ));
        }
    }
    if c.stop_time <= c.start_time {
        out.push(Diagnostic::error(
            E_INVALID_CONFIG,
            "stop_time must be greater than start_time",
            loc(),
        ));
    }
    if c.output_step <= 0.0 {
        out.push(Diagnostic::error(
            E_INVALID_CONFIG,
            "output_step must be positive",
            loc(),
        ));
    }
    if c.rel_tol <= 0.0 {
        out.push(Diagnostic::error(
            E_INVALID_CONFIG,
            "rel_tol must be positive",
            loc(),
        ));
    }
    if c.abs_tol <= 0.0 {
        out.push(Diagnostic::error(
            E_INVALID_CONFIG,
            "abs_tol must be positive",
            loc(),
        ));
    }
    if let Some(v) = c.max_step {
        if v <= 0.0 || !v.is_finite() {
            out.push(Diagnostic::error(
                E_INVALID_CONFIG,
                "max_step must be positive",
                loc(),
            ));
        }
    }
}

fn check_sids(
    sys: &System,
    path: &SystemPath,
    seen: &mut BTreeSet<BlockId>,
    out: &mut Vec<Diagnostic>,
) {
    for b in &sys.blocks {
        if !seen.insert(b.sid) {
            out.push(Diagnostic::error(
                E_DUPLICATE_SID,
                format!("SID {} is used by more than one block", b.sid),
                Location::block(path, b.sid),
            ));
        }
    }
    for b in &sys.blocks {
        if let Some(c) = &b.system {
            check_sids(c, &path.child(b.sid), seen, out);
        }
    }
}

fn dangling(bs: &[Branch]) -> bool {
    bs.iter()
        .any(|b| (b.dst.is_none() && b.branches.is_empty()) || dangling(&b.branches))
}

/// The `Port` number of an Inport / Outport, when it is a usable positive integer.
///
/// Values that are not positive integers are reported as `E_INVALID_PARAM` by
/// `validate_param`, so they are skipped here to avoid a second diagnostic.
fn port_number(b: &Block) -> Option<u32> {
    b.params
        .get("Port")?
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|n| *n > 0)
}

/// Inport / Outport port numbers of one system: unique and `1..=count` without gaps.
///
/// The subsystem port order depends on these numbers, so a duplicate or a gap silently
/// rewires the parent system (#196).
fn check_port_numbers(sys: &System, path: &SystemPath, out: &mut Vec<Diagnostic>) {
    for block_type in ["Inport", "Outport"] {
        let mut numbers: Vec<(u32, BlockId)> = vec![];
        let mut seen = BTreeSet::new();
        for b in sys.blocks.iter().filter(|b| b.block_type == block_type) {
            let Some(n) = port_number(b) else { continue };
            if seen.insert(n) {
                numbers.push((n, b.sid));
            } else {
                out.push(Diagnostic::error(
                    E_DUPLICATE_PORT_NUMBER,
                    format!("port number {n} is used by more than one {block_type}"),
                    Location::block(path, b.sid),
                ));
            }
        }
        numbers.sort_unstable();
        let count = numbers.len() as u32;
        for (index, (n, sid)) in numbers.iter().enumerate() {
            let expected = index as u32 + 1;
            if *n != expected {
                out.push(Diagnostic::error(
                    E_PORT_NUMBER_GAP,
                    format!("{block_type} port numbers must be 1..{count} without gaps"),
                    Location::block(path, *sid),
                ));
            }
        }
    }
}

fn check_system(
    sys: &System,
    path: &SystemPath,
    seen_lines: &mut BTreeSet<LineId>,
    out: &mut Vec<Diagnostic>,
) {
    let mut names = BTreeSet::new();
    let mut counts: BTreeMap<BlockId, PortCounts> = BTreeMap::new();
    for b in &sys.blocks {
        let loc = Location::block(path, b.sid);
        if !names.insert(b.name.clone()) {
            out.push(Diagnostic::error(
                E_DUPLICATE_BLOCK_NAME,
                format!("block name '{}' is used more than once", b.name),
                loc.clone(),
            ));
        }
        if b.name.is_empty() {
            out.push(Diagnostic::error(
                E_INVALID_NAME,
                "name must not be empty",
                loc.clone(),
            ));
        } else if has_control_chars(&b.name) {
            out.push(Diagnostic::error(
                E_INVALID_NAME,
                "name must not contain control characters",
                loc.clone(),
            ));
        }
        let Some(s) = spec(&b.block_type) else {
            out.push(Diagnostic::error(
                E_UNKNOWN_BLOCK_TYPE,
                format!("unknown block type '{}'", b.block_type),
                loc,
            ));
            counts.insert(
                b.sid,
                port_counts(b).unwrap_or(PortCounts {
                    inputs: 0,
                    outputs: 0,
                }),
            );
            continue;
        };
        for ps in s.params {
            match b.params.get(ps.name) {
                None if b.block_type == "Goto" && ps.name == "TagVisibility" => {}
                None => out.push(Diagnostic::error(
                    E_INVALID_PARAM,
                    format!("parameter '{}' is missing", ps.name),
                    loc.clone(),
                )),
                Some(v) => {
                    if let Err(e) = validate_param(ps.kind, v) {
                        out.push(Diagnostic::error(
                            E_INVALID_PARAM,
                            format!("parameter '{}': {e}", ps.name),
                            loc.clone(),
                        ));
                    }
                }
            }
        }
        counts.insert(
            b.sid,
            port_counts(b).unwrap_or(PortCounts {
                inputs: 0,
                outputs: 0,
            }),
        );
    }
    for l in &sys.lines {
        if !seen_lines.insert(l.id) {
            out.push(Diagnostic::error(
                E_DUPLICATE_LINE_ID,
                format!("line id {} is used more than once", l.id),
                Location::line(path, l.id),
            ));
        }
    }
    let mut dst_count: BTreeMap<PortRef, usize> = BTreeMap::new();
    let mut used_src: BTreeSet<PortRef> = BTreeSet::new();
    for l in &sys.lines {
        check_line(sys, path, l, &counts, &mut dst_count, &mut used_src, out);
    }
    for (p, n) in &dst_count {
        if *n >= 2 {
            let name = sys
                .block(p.block)
                .map(|b| b.name.clone())
                .unwrap_or_default();
            out.push(Diagnostic::error(
                E_INPUT_MULTIPLY_DRIVEN,
                format!(
                    "input port {} of '{}' has more than one source",
                    p.port, name
                ),
                Location::block(path, p.block),
            ));
        }
    }
    for b in &sys.blocks {
        if spec(&b.block_type).is_none() {
            continue;
        }
        let c = counts[&b.sid];
        for port in 1..=c.inputs {
            if !dst_count.contains_key(&PortRef { block: b.sid, port }) {
                out.push(Diagnostic::warning(
                    W_UNCONNECTED_INPUT,
                    format!(
                        "input port {port} of '{}' is not connected; it will read 0",
                        b.name
                    ),
                    Location::block(path, b.sid),
                ));
            }
        }
    }
    for b in &sys.blocks {
        if spec(&b.block_type).is_none() {
            continue;
        }
        let c = counts[&b.sid];
        for port in 1..=c.outputs {
            if !used_src.contains(&PortRef { block: b.sid, port }) {
                out.push(Diagnostic::warning(
                    W_UNCONNECTED_OUTPUT,
                    format!("output port {port} of '{}' is not connected", b.name),
                    Location::block(path, b.sid),
                ));
            }
        }
    }
    for b in &sys.blocks {
        if let Some(c) = &b.system {
            check_system(c, &path.child(b.sid), seen_lines, out);
        }
    }
    check_port_numbers(sys, path, out);
}

fn check_line(
    sys: &System,
    path: &SystemPath,
    l: &Line,
    counts: &BTreeMap<BlockId, PortCounts>,
    dst_count: &mut BTreeMap<PortRef, usize>,
    used_src: &mut BTreeSet<PortRef>,
    out: &mut Vec<Diagnostic>,
) {
    let loc = Location::line(path, l.id);
    if has_control_chars(&l.name) {
        out.push(Diagnostic::error(
            E_INVALID_NAME,
            "name must not contain control characters",
            loc.clone(),
        ));
    }
    match sys.block(l.src.block) {
        None => out.push(Diagnostic::error(
            E_LINE_SRC_MISSING,
            format!("line {} starts at missing block {}", l.id, l.src.block),
            loc.clone(),
        )),
        Some(b) => {
            let n = counts.get(&b.sid).map(|c| c.outputs).unwrap_or(0);
            if l.src.port == 0 || l.src.port > n {
                out.push(Diagnostic::error(
                    E_PORT_OUT_OF_RANGE,
                    format!("output port {} of '{}' does not exist", l.src.port, b.name),
                    loc.clone(),
                ));
            } else {
                used_src.insert(l.src);
            }
        }
    }
    for d in l.destinations() {
        match sys.block(d.block) {
            None => out.push(Diagnostic::error(
                E_LINE_DST_MISSING,
                format!("line {} ends at missing block {}", l.id, d.block),
                loc.clone(),
            )),
            Some(b) => {
                let n = counts.get(&b.sid).map(|c| c.inputs).unwrap_or(0);
                if d.port == 0 || d.port > n {
                    out.push(Diagnostic::error(
                        E_PORT_OUT_OF_RANGE,
                        format!("input port {} of '{}' does not exist", d.port, b.name),
                        loc.clone(),
                    ));
                } else {
                    *dst_count.entry(d).or_insert(0) += 1;
                }
            }
        }
    }
    if (l.dst.is_none() && l.branches.is_empty()) || dangling(&l.branches) {
        out.push(Diagnostic::warning(
            W_DANGLING_LINE,
            format!("line {} has an unconnected end", l.id),
            loc,
        ));
    }
}
