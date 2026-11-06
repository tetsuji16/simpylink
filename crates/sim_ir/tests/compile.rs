//! Compiler tests (Issue #58, #72) including subsystem flattening.
use sim_core::geom::Rect;
use sim_core::ids::{BlockId, LineId};
use sim_core::json::load_from_path;
use sim_core::library::{fill_default_params, new_block};
use sim_core::model::*;
use sim_ir::*;

fn fx(name: &str) -> Model {
    load_from_path(&std::path::PathBuf::from(format!(
        "{}/../../tests/fixtures/models/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    )))
    .unwrap()
}

#[test]
fn step_gain_scope_ir() {
    let c = compile(&fx("step_gain_scope")).unwrap();
    assert!(c.diagnostics.is_empty());
    let ir = c.ir;
    assert_eq!(
        ir.blocks.iter().map(|b| b.sid.0).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        ir.blocks[0].kind,
        IrBlockKind::Step {
            time: 1.0,
            before: 0.0,
            after: 2.0
        }
    );
    assert_eq!(ir.blocks[1].inputs, vec![Some(SignalId(0))]);
    assert_eq!(ir.blocks[2].var_base, "scope_3");
    assert_eq!(
        ir.signals
            .iter()
            .map(|s| s.var_name.as_str())
            .collect::<Vec<_>>(),
        vec!["step_1", "gain_2"]
    );
    assert_eq!(ir.breakpoints, vec![1.0]);
    assert_eq!(
        ir.logs,
        vec![IrLog {
            name: "Scope/1".into(),
            block_sid: BlockId(3),
            port: 1,
            signal: Some(SignalId(1))
        }]
    );
    assert_eq!(ir.config.solver_method, "RK45");
    assert_eq!(ir.blocks[1].sample_time, SampleTime::Continuous);
}

#[test]
fn ordering_states_and_errors() {
    let ir = compile(&fx("first_order_lag")).unwrap().ir;
    assert_eq!(
        ir.blocks.iter().map(|b| b.sid.0).collect::<Vec<_>>(),
        vec![1, 3, 2, 4]
    );
    assert_eq!(ir.blocks[1].cont_state, Some(0..1));
    assert_eq!(ir.num_cont_states, 1);
    let ir = compile(&fx("gain_constant")).unwrap().ir;
    assert_eq!(ir.blocks[1].sample_time, SampleTime::Constant);
    let mut m = fx("sum_constants");
    m.root.lines.retain(|l| l.id != LineId(2));
    let c = compile(&m).unwrap();
    assert_eq!(c.ir.blocks[2].inputs, vec![Some(SignalId(0)), None]);
    assert_eq!(
        c.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        vec!["W_UNCONNECTED_INPUT", "W_UNCONNECTED_OUTPUT"]
    );
    let f = compile(&fx("algebraic_loop")).unwrap_err();
    let loops: Vec<_> = f
        .diagnostics
        .iter()
        .filter(|d| d.code == "E_ALGEBRAIC_LOOP")
        .collect();
    assert_eq!(loops.len(), 1);
    assert_eq!(
        loops[0].message,
        "algebraic loop: Gain(2) -> Sum(3) -> Gain(2)"
    );
    let mut m = fx("step_gain_scope");
    m.config.solver = SolverKind::Ode15s;
    assert_eq!(compile(&m).unwrap().ir.config.solver_method, "BDF");
    let mut m = fx("step_gain_scope");
    m.root.blocks[0].params.insert("Time".into(), "5".into());
    assert!(compile(&m).unwrap().ir.breakpoints.is_empty());
}

/// Constant(1) → Subsystem(2){ In1(3) → Gain(4, 3) → Out1(5) } → Scope(6)
pub fn subsystem_model() -> Model {
    let mut m = Model::new("subsystem_gain");
    m.config.stop_time = 1.0;
    m.config.output_step = 0.5;
    let mut inner = System::default();
    let add = |sys: &mut System, t: &str, sid: u32, x: f64| {
        let mut b = new_block(sys, t, BlockId(sid), sim_core::geom::Point::new(x, 50.0)).unwrap();
        fill_default_params(&mut b);
        sys.insert_block_sorted(b);
    };
    add(&mut inner, "Inport", 3, 20.0);
    add(&mut inner, "Gain", 4, 80.0);
    inner
        .block_mut(BlockId(4))
        .unwrap()
        .params
        .insert("Gain".into(), "3".into());
    add(&mut inner, "Outport", 5, 140.0);
    let pr = |b, p| PortRef {
        block: BlockId(b),
        port: p,
    };
    let ln = |id, s: PortRef, d: PortRef| Line {
        id: LineId(id),
        src: s,
        points: vec![],
        dst: Some(d),
        branches: vec![],
        name: String::new(),
    };
    inner.lines = vec![ln(3, pr(3, 1), pr(4, 1)), ln(4, pr(4, 1), pr(5, 1))];
    let mut root = System::default();
    add(&mut root, "Constant", 1, 20.0);
    root.blocks[0].params.insert("Value".into(), "2".into());
    root.insert_block_sorted(Block {
        sid: BlockId(2),
        block_type: "SubSystem".into(),
        name: "Subsystem".into(),
        position: Rect::new(80.0, 40.0, 140.0, 80.0),
        orientation: Orientation::Right,
        params: Default::default(),
        system: Some(inner),
    });
    add(&mut root, "Scope", 6, 200.0);
    root.lines = vec![ln(1, pr(1, 1), pr(2, 1)), ln(2, pr(2, 1), pr(6, 1))];
    m.root = root;
    m.next_sid = 7;
    m.next_line_id = 5;
    m
}

#[test]
fn subsystem_flattening() {
    let m = subsystem_model();
    assert!(
        sim_core::validate::validate_model(&m).is_empty(),
        "{:?}",
        sim_core::validate::validate_model(&m)
    );
    let ir = compile(&m).unwrap().ir;
    assert_eq!(
        ir.blocks.iter().map(|b| b.sid.0).collect::<Vec<_>>(),
        vec![1, 4, 6]
    );
    assert_eq!(ir.blocks[1].inputs, vec![Some(SignalId(0))]);
    assert_eq!(ir.blocks[2].inputs, vec![Some(SignalId(1))]);
}
