//! Hit testing with zoom-independent radii.
use super::geometry::{block_ports, distance_to_segment, LINE_HIT_DISTANCE_PX, PORT_HIT_RADIUS_PX};
use super::route::{all_paths, PathNode};
use super::view::{ScreenPos, ViewTransform};
use sim_core::geom::Rect;
use sim_core::ids::{BlockId, LineId};
use sim_core::model::{PortRef, System};

/// What is under the pointer.
#[derive(Debug, Clone, PartialEq)]
pub enum HitTarget {
    OutputPort(PortRef),
    InputPort(PortRef),
    Block(BlockId),
    Line {
        line: LineId,
        branch: Vec<usize>,
        segment: u32,
        t: f64,
    },
    Empty,
}

/// Hit test at a screen position.
pub fn hit_test(sys: &System, view: &ViewTransform, pos: ScreenPos) -> HitTarget {
    let p = view.to_model(pos);
    let z = view.zoom as f64;
    let (rp, rl) = (
        PORT_HIT_RADIUS_PX as f64 / z,
        LINE_HIT_DISTANCE_PX as f64 / z,
    );
    for outputs in [true, false] {
        let mut best: Option<(f64, PortRef)> = None;
        for b in &sys.blocks {
            let ports = block_ports(b);
            let list = if outputs {
                &ports.outputs
            } else {
                &ports.inputs
            };
            for (i, q) in list.iter().enumerate() {
                let d = p.distance(*q);
                if d <= rp && best.map(|(bd, _)| d <= bd + 1e-9).unwrap_or(true) {
                    best = Some((
                        d,
                        PortRef {
                            block: b.sid,
                            port: i as u32 + 1,
                        },
                    ));
                }
            }
        }
        if let Some((_, pr)) = best {
            return if outputs {
                HitTarget::OutputPort(pr)
            } else {
                HitTarget::InputPort(pr)
            };
        }
    }
    if let Some(b) = sys.blocks.iter().rev().find(|b| b.position.contains(p)) {
        return HitTarget::Block(b.sid);
    }
    let mut best: Option<(f64, HitTarget)> = None;
    fn walk(
        n: &PathNode,
        path: &mut Vec<usize>,
        line: LineId,
        p: sim_core::geom::Point,
        rl: f64,
        best: &mut Option<(f64, HitTarget)>,
    ) {
        for (s, w) in n.points.windows(2).enumerate() {
            let (d, t) = distance_to_segment(p, w[0], w[1]);
            if d <= rl && best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                *best = Some((
                    d,
                    HitTarget::Line {
                        line,
                        branch: path.clone(),
                        segment: s as u32,
                        t,
                    },
                ));
            }
        }
        for (i, c) in n.children.iter().enumerate() {
            path.push(i);
            walk(c, path, line, p, rl, best);
            path.pop();
        }
    }
    for lp in all_paths(sys) {
        walk(&lp.root, &mut vec![], lp.line, p, rl, &mut best);
    }
    best.map(|b| b.1).unwrap_or(HitTarget::Empty)
}

/// Blocks intersecting a model rectangle (sid order).
pub fn blocks_in_rect(sys: &System, r: Rect) -> Vec<BlockId> {
    sys.blocks
        .iter()
        .filter(|b| b.position.intersects(&r))
        .map(|b| b.sid)
        .collect()
}

/// True if the input port is a destination of a line.
pub fn input_connected(sys: &System, port: PortRef) -> bool {
    sys.lines.iter().any(|l| l.destinations().contains(&port))
}
