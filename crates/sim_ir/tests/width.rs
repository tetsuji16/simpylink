//! Signal width propagation tests (Issue #161 §H/§J).
use sim_core::diag::codes::{E_DIMENSION, E_UNSUPPORTED_FEATURE};
use sim_core::geom::Point;
use sim_core::ids::{BlockId, LineId};
use sim_core::library::new_block;
use sim_core::model::{Block, Line, Model, PortRef, System};
use sim_core::workspace::Workspace;
use sim_ir::graph::{build_connectivity, flat_blocks};
use sim_ir::lower::lower_block_in;
use sim_ir::width::propagate_widths;
use std::collections::BTreeMap;

fn blk(sid: u32, block_type: &str, params: &[(&str, &str)]) -> Block {
    let mut b = new_block(
        &System::default(),
        block_type,
        BlockId(sid),
        Point::new(0.0, 0.0),
    )
    .unwrap_or_else(|e| panic!("{block_type}: {e}"));
    b.name = format!("{block_type}{sid}");
    for (k, v) in params {
        b.params.insert((*k).into(), (*v).into());
    }
    b
}

fn pr(block: u32, port: u32) -> PortRef {
    PortRef {
        block: BlockId(block),
        port,
    }
}

/// Builds a model from blocks (sid order) and lines `(src, src port, dst, dst port)`.
fn model(blocks: Vec<Block>, lines: &[(u32, u32, u32, u32)]) -> Model {
    let mut m = Model::new("width");
    m.config.stop_time = 1.0;
    m.config.output_step = 0.5;
    m.root.blocks = blocks;
    m.root.lines = lines
        .iter()
        .enumerate()
        .map(|(i, (s, sp, d, dp))| Line {
            id: LineId(i as u32 + 1),
            src: pr(*s, *sp),
            points: vec![],
            dst: Some(pr(*d, *dp)),
            branches: vec![],
            name: String::new(),
        })
        .collect();
    m
}

fn widths(m: &Model) -> Result<BTreeMap<PortRef, usize>, Vec<String>> {
    let conn = build_connectivity(m);
    let flat = flat_blocks(m);
    let mut lowered = BTreeMap::new();
    for fb in &flat {
        let l = lower_block_in(fb.block, fb.path.is_root(), &fb.path, &Workspace::new())
            .unwrap_or_else(|d| panic!("{}: {d}", fb.block.name));
        lowered.insert(fb.block.sid, (l, fb));
    }
    propagate_widths(&conn, &lowered).map_err(|ds| ds.into_iter().map(|d| d.message).collect())
}

fn w(m: &Model, block: u32, port: u32) -> usize {
    widths(m).unwrap()[&pr(block, port)]
}

fn code(m: &Model) -> Vec<String> {
    sim_ir::compile(m)
        .err()
        .map(|f| f.diagnostics.into_iter().map(|d| d.code).collect())
        .unwrap_or_default()
}

#[test]
fn constant_gain_vector_keeps_the_width() {
    let m = model(
        vec![
            blk(1, "Constant", &[("Value", "[1 2 3]")]),
            blk(2, "Gain", &[("Gain", "2")]),
            blk(3, "Scope", &[]),
        ],
        &[(1, 1, 2, 1), (2, 1, 3, 1)],
    );
    assert_eq!(w(&m, 1, 1), 3);
    assert_eq!(w(&m, 2, 1), 3);
    assert!(code(&m).contains(&E_UNSUPPORTED_FEATURE.to_string()));
}

#[test]
fn element_wise_blocks_take_the_widest_input() {
    let m = model(
        vec![
            blk(1, "Constant", &[("Value", "[1 2]")]),
            blk(2, "Constant", &[("Value", "1")]),
            blk(3, "Sum", &[("Inputs", "++")]),
            blk(4, "Scope", &[]),
        ],
        &[(1, 1, 3, 1), (2, 1, 3, 2), (3, 1, 4, 1)],
    );
    assert_eq!(w(&m, 3, 1), 2);
}

#[test]
fn mux_and_demux_split_widths() {
    let m = model(
        vec![
            blk(1, "Constant", &[("Value", "1")]),
            blk(2, "Constant", &[("Value", "[2 3]")]),
            blk(3, "Mux", &[("Inputs", "2")]),
            blk(4, "Demux", &[("Outputs", "3")]),
            blk(5, "Scope", &[("NumInputPorts", "3")]),
        ],
        &[
            (1, 1, 3, 1),
            (2, 1, 3, 2),
            (3, 1, 4, 1),
            (4, 1, 5, 1),
            (4, 2, 5, 2),
            (4, 3, 5, 3),
        ],
    );
    assert_eq!(w(&m, 3, 1), 3);
    for port in 1..=3 {
        assert_eq!(w(&m, 4, port), 1);
    }
    assert!(code(&m).contains(&E_UNSUPPORTED_FEATURE.to_string()));
}

#[test]
fn scalar_mux_and_demux_fill_their_widths_in_the_ir() {
    let m = model(
        vec![
            blk(1, "Constant", &[("Value", "2")]),
            blk(2, "Mux", &[("Inputs", "1")]),
            blk(3, "Demux", &[("Outputs", "1")]),
            blk(4, "Scope", &[]),
        ],
        &[(1, 1, 2, 1), (2, 1, 3, 1), (3, 1, 4, 1)],
    );
    let ir = sim_ir::compile(&m)
        .unwrap_or_else(|f| panic!("{:?}", f.diagnostics))
        .ir;
    let mux = ir
        .blocks
        .iter()
        .find(|b| matches!(b.kind, sim_ir::IrBlockKind::Mux { .. }))
        .unwrap();
    assert_eq!(mux.kind, sim_ir::IrBlockKind::Mux { widths: vec![1] });
    let demux = ir
        .blocks
        .iter()
        .find(|b| matches!(b.kind, sim_ir::IrBlockKind::Demux { .. }))
        .unwrap();
    assert_eq!(demux.kind, sim_ir::IrBlockKind::Demux { widths: vec![1] });
}

#[test]
fn demux_with_explicit_output_widths() {
    let m = model(
        vec![
            blk(1, "Constant", &[("Value", "[1 2 3 4]")]),
            blk(2, "Demux", &[("Outputs", "[1 3]")]),
            blk(3, "Scope", &[("NumInputPorts", "2")]),
        ],
        &[(1, 1, 2, 1), (2, 1, 3, 1), (2, 2, 3, 2)],
    );
    assert_eq!(w(&m, 2, 1), 1);
    assert_eq!(w(&m, 2, 2), 3);
}

#[test]
fn feedback_width_comes_from_the_initial_condition() {
    let m = model(
        vec![
            blk(1, "Constant", &[("Value", "[1 2]")]),
            blk(2, "Sum", &[("Inputs", "+-")]),
            blk(
                3,
                "UnitDelay",
                &[("InitialCondition", "0"), ("SampleTime", "1")],
            ),
            blk(4, "Scope", &[]),
        ],
        &[(1, 1, 2, 1), (2, 1, 3, 1), (3, 1, 2, 2), (3, 1, 4, 1)],
    );
    assert_eq!(w(&m, 2, 1), 2);
    assert_eq!(w(&m, 3, 1), 2);
}

#[test]
fn mismatched_widths_are_reported() {
    let m = model(
        vec![
            blk(1, "Constant", &[("Value", "[1 2]")]),
            blk(2, "Constant", &[("Value", "[1 2 3]")]),
            blk(3, "Sum", &[("Inputs", "++")]),
            blk(4, "Scope", &[]),
        ],
        &[(1, 1, 3, 1), (2, 1, 3, 2), (3, 1, 4, 1)],
    );
    let errs = widths(&m).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| e.contains("input 2 has width 3, expected 1 or 2")),
        "{errs:?}"
    );
    assert!(code(&m).contains(&E_DIMENSION.to_string()));
}

#[test]
fn demux_must_split_evenly() {
    let m = model(
        vec![
            blk(1, "Constant", &[("Value", "[1 2 3]")]),
            blk(2, "Demux", &[("Outputs", "2")]),
            blk(3, "Scope", &[("NumInputPorts", "2")]),
        ],
        &[(1, 1, 2, 1), (2, 1, 3, 1), (2, 2, 3, 2)],
    );
    let errs = widths(&m).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| e.contains("cannot split width 3 into 2 outputs")),
        "{errs:?}"
    );
}

#[test]
fn vector_input_to_scalar_blocks_is_rejected() {
    let m = model(
        vec![
            blk(1, "Constant", &[("Value", "[1 2]")]),
            blk(
                2,
                "TransferFcn",
                &[("Numerator", "[1]"), ("Denominator", "[1 1]")],
            ),
            blk(3, "Scope", &[]),
        ],
        &[(1, 1, 2, 1), (2, 1, 3, 1)],
    );
    let failure = sim_ir::compile(&m).unwrap_err();
    assert!(
        failure
            .diagnostics
            .iter()
            .any(|d| d.code == E_UNSUPPORTED_FEATURE
                && d.message
                    .contains("vector input to 'TransferFcn2' is not supported")),
        "{:?}",
        failure.diagnostics
    );
}

#[test]
fn matrix_signals_are_rejected() {
    let m = model(
        vec![
            blk(1, "Constant", &[("Value", "[1 2; 3 4]")]),
            blk(2, "Scope", &[]),
        ],
        &[(1, 1, 2, 1)],
    );
    let failure = sim_ir::compile(&m).unwrap_err();
    assert!(
        failure
            .diagnostics
            .iter()
            .any(|d| d.code == E_UNSUPPORTED_FEATURE && d.message.contains("matrix signals")),
        "{:?}",
        failure.diagnostics
    );
}

#[test]
fn scalar_models_keep_width_one() {
    for name in [
        "step_gain_scope",
        "sum_constants",
        "first_order_lag",
        "accumulator",
    ] {
        let path = format!(
            "{}/../../tests/fixtures/models/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let m = sim_core::json::load_from_path(std::path::Path::new(&path))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let ir = sim_ir::compile(&m).unwrap_or_else(|f| panic!("{name}: {:?}", f.diagnostics));
        assert!(ir.ir.signals.iter().all(|s| s.width == 1), "{name}");
    }
}
