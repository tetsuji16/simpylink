//! Two Unit Delays define two discrete rates (Issue #75 §J).
use sim_core::geom::Point;
use sim_core::ids::{BlockId, LineId};
use sim_core::library::{fill_default_params, new_block};
use sim_core::model::*;
use sim_ir::*;

fn push_block(sys: &mut System, block_type: &str, sid: u32, x: f64) {
    let mut b = new_block(sys, block_type, BlockId(sid), Point::new(x, 50.0)).unwrap();
    fill_default_params(&mut b);
    sys.insert_block_sorted(b);
}

fn line(id: u32, src: u32, dst: u32) -> Line {
    Line {
        id: LineId(id),
        src: PortRef {
            block: BlockId(src),
            port: 1,
        },
        points: vec![],
        dst: Some(PortRef {
            block: BlockId(dst),
            port: 1,
        }),
        branches: vec![],
        name: String::new(),
    }
}

#[test]
fn two_unit_delays_define_two_discrete_rates() {
    let mut m = Model::new("two_delays");
    m.config.stop_time = 2.0;
    let mut root = System::default();
    push_block(&mut root, "Constant", 1, 20.0);
    push_block(&mut root, "UnitDelay", 2, 80.0);
    push_block(&mut root, "Scope", 3, 200.0);
    push_block(&mut root, "UnitDelay", 5, 140.0);
    root.block_mut(BlockId(2))
        .unwrap()
        .params
        .insert("SampleTime".into(), "1".into());
    root.block_mut(BlockId(5))
        .unwrap()
        .params
        .insert("SampleTime".into(), "2".into());
    root.lines = vec![line(1, 1, 2), line(2, 2, 5), line(3, 5, 3)];
    m.root = root;
    m.next_sid = 6;
    m.next_line_id = 4;
    let ir = compile(&m).unwrap().ir;
    assert_eq!(ir.discrete_rates, vec![(1.0, 0.0), (2.0, 0.0)]);
    let rate = |sid: u32| {
        ir.blocks
            .iter()
            .find(|b| b.sid == BlockId(sid))
            .unwrap()
            .rate_index
    };
    assert_eq!(rate(2), Some(0));
    assert_eq!(rate(5), Some(1));
}
