//! Continuous state allocation order (Issue #71).
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
fn two_integrators_get_sid_order_offsets() {
    let mut m = Model::new("two_integrators");
    m.config.stop_time = 1.0;
    let mut root = System::default();
    push_block(&mut root, "Constant", 1, 20.0);
    push_block(&mut root, "Integrator", 2, 80.0);
    push_block(&mut root, "Scope", 3, 200.0);
    push_block(&mut root, "Integrator", 5, 140.0);
    root.lines = vec![line(1, 1, 5), line(2, 5, 2), line(3, 2, 3)];
    m.root = root;
    m.next_sid = 6;
    m.next_line_id = 4;
    let ir = compile(&m).unwrap().ir;
    let at = |sid: u32| {
        ir.blocks
            .iter()
            .find(|b| b.sid == BlockId(sid))
            .unwrap()
            .cont_state
            .clone()
    };
    assert_eq!(at(2), Some(0..1));
    assert_eq!(at(5), Some(1..2));
    assert_eq!(ir.num_cont_states, 2);
}
