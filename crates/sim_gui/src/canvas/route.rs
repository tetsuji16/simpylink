//! Line polylines: automatic orthogonal routing and branch taps.
use super::geometry::{block_ports, out_dir};
use sim_core::geom::Point;
use sim_core::ids::LineId;
use sim_core::model::{Branch, Line, Orientation, PortRef, System, TapPoint};

pub const STUB: f64 = 15.0;
pub const FEEDBACK_GAP: f64 = 20.0;
pub const DANGLING_STUB: f64 = 20.0;
/// Vertical space reserved under a block for its name label.
pub const NAME_CLEARANCE: f64 = 18.0;

/// y coordinate for the horizontal run of a feedback (backwards) line between
/// x = `x0` and `x1`: below `base + FEEDBACK_GAP` and below the name labels of
/// the blocks in that span that sit in the same row (so the line does not
/// cross block names, issue #44).
///
/// The route's source and destination are already covered by
/// `base + FEEDBACK_GAP`, so other blocks in the span can push the line down.
fn feedback_level(
    sys: &System,
    x0: f64,
    x1: f64,
    base: f64,
    source: Option<sim_core::ids::BlockId>,
    destination: Option<sim_core::ids::BlockId>,
) -> f64 {
    let (lo, hi) = (x0.min(x1) - STUB, x0.max(x1) + STUB);
    let mut y = base + FEEDBACK_GAP;
    for b in &sys.blocks {
        let r = b.position;
        let is_endpoint = Some(b.sid) == source || Some(b.sid) == destination;
        if r.right >= lo && r.left <= hi && r.top <= base + FEEDBACK_GAP && !is_endpoint {
            y = y.max(r.bottom + NAME_CLEARANCE + 6.0);
        }
    }
    y.round()
}

/// One polyline of a line tree.
#[derive(Debug, Clone, PartialEq)]
pub struct PathNode {
    pub points: Vec<Point>,
    pub dst: Option<PortRef>,
    pub children: Vec<PathNode>,
}

/// Rendered path of a line.
#[derive(Debug, Clone, PartialEq)]
pub struct LinePath {
    pub line: LineId,
    pub root: PathNode,
}

/// Orthogonal route from `a` (output) to `b` (input) for right-facing ports.
pub fn auto_route(a: Point, b: Point, below: f64) -> Vec<Point> {
    if (a.y - b.y).abs() < 0.5 && b.x > a.x {
        return vec![a, b];
    }
    if b.x >= a.x + 2.0 * STUB {
        let xm = ((a.x + b.x) / 2.0).round();
        return vec![a, Point::new(xm, a.y), Point::new(xm, b.y), b];
    }
    vec![
        a,
        Point::new(a.x + STUB, a.y),
        Point::new(a.x + STUB, below),
        Point::new(b.x - STUB, below),
        Point::new(b.x - STUB, b.y),
        b,
    ]
}

/// Route between arbitrary port directions (generic L-shape with stubs).
pub fn auto_route_dirs(
    a: Point,
    aout: (f64, f64),
    b: Point,
    bin: (f64, f64),
    below: f64,
) -> Vec<Point> {
    if aout == (1.0, 0.0) && bin == (1.0, 0.0) {
        return auto_route(a, b, below);
    }
    let a1 = Point::new(a.x + aout.0 * STUB, a.y + aout.1 * STUB);
    let b1 = Point::new(b.x - bin.0 * STUB, b.y - bin.1 * STUB);
    let corner = if aout.0 != 0.0 {
        Point::new(b1.x, a1.y)
    } else {
        Point::new(a1.x, b1.y)
    };
    let mut v = vec![a, a1, corner, b1, b];
    v.dedup_by(|p, q| (p.x - q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9);
    v
}

/// Point on a polyline.
pub fn tap_position(points: &[Point], tap: TapPoint) -> Point {
    if points.is_empty() {
        return Point::default();
    }
    if points.len() == 1 {
        return points[0];
    }
    let n = points.len() - 1;
    let s = (tap.segment as usize).min(n - 1);
    let t = tap.t.clamp(0.0, 1.0);
    let (a, b) = (points[s], points[s + 1]);
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn input_info(sys: &System, p: PortRef) -> Option<(Point, (f64, f64), f64)> {
    let b = sys.block(p.block)?;
    let pos = *block_ports(b).inputs.get(p.port.checked_sub(1)? as usize)?;
    Some((pos, out_dir(b.orientation), b.position.bottom))
}

#[derive(Clone, Copy)]
struct RouteStart {
    block: Option<sim_core::ids::BlockId>,
    point: Point,
    direction: (f64, f64),
    bottom: f64,
}

fn node(
    sys: &System,
    start: RouteStart,
    points: &[Point],
    dst: Option<PortRef>,
    branches: &[Branch],
) -> PathNode {
    let RouteStart {
        block: source,
        point: start,
        direction: sdir,
        bottom: sbottom,
    } = start;
    let target = dst.and_then(|d| input_info(sys, d));
    let pts = match (points.is_empty(), target) {
        (false, Some((p, _, _))) => {
            let mut v = vec![start];
            v.extend_from_slice(points);
            v.push(p);
            v
        }
        (false, None) => {
            let mut v = vec![start];
            v.extend_from_slice(points);
            v
        }
        (true, Some((p, din, bot))) => {
            let destination = dst
                .filter(|p| sys.block(p.block).is_some())
                .map(|p| p.block);
            let below = feedback_level(sys, start.x, p.x, sbottom.max(bot), source, destination);
            auto_route_dirs(start, sdir, p, din, below)
        }
        (true, None) => vec![
            start,
            Point::new(
                start.x + sdir.0 * DANGLING_STUB,
                start.y + sdir.1 * DANGLING_STUB,
            ),
        ],
    };
    let children = branches
        .iter()
        .map(|br| {
            let s = tap_position(&pts, br.tap);
            node(
                sys,
                RouteStart {
                    block: None,
                    point: s,
                    direction: (1.0, 0.0),
                    bottom: s.y,
                },
                &br.points,
                br.dst,
                &br.branches,
            )
        })
        .collect();
    PathNode {
        points: pts,
        dst: if target.is_some() { dst } else { None },
        children,
    }
}

/// Path of one line (None if its source block does not exist).
pub fn line_path(sys: &System, line: &Line) -> Option<LinePath> {
    let b = sys.block(line.src.block)?;
    let start = *block_ports(b)
        .outputs
        .get(line.src.port.checked_sub(1)? as usize)?;
    let dir = if b.orientation == Orientation::Right {
        (1.0, 0.0)
    } else {
        out_dir(b.orientation)
    };
    Some(LinePath {
        line: line.id,
        root: node(
            sys,
            RouteStart {
                block: Some(line.src.block),
                point: start,
                direction: dir,
                bottom: b.position.bottom,
            },
            &line.points,
            line.dst,
            &line.branches,
        ),
    })
}

/// Paths of all lines in order.
pub fn all_paths(sys: &System) -> Vec<LinePath> {
    sys.lines.iter().filter_map(|l| line_path(sys, l)).collect()
}

/// Node at a branch path.
pub fn node_at<'a>(root: &'a PathNode, branch: &[usize]) -> Option<&'a PathNode> {
    let mut n = root;
    for i in branch {
        n = n.children.get(*i)?;
    }
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::geom::Rect;
    use sim_core::ids::BlockId;
    use sim_core::model::Block;

    fn block(sid: u32, block_type: &str, r: Rect, params: &[(&str, &str)]) -> Block {
        Block {
            sid: BlockId(sid),
            block_type: block_type.into(),
            name: format!("G{sid}"),
            position: r,
            orientation: Orientation::Right,
            params: params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            system: None,
        }
    }

    fn blk(sid: u32, r: Rect) -> Block {
        block(sid, "Gain", r, &[])
    }

    fn line(id: u32, src: PortRef, dst: Option<PortRef>, branches: Vec<Branch>) -> Line {
        Line {
            id: LineId(id),
            src,
            points: vec![],
            dst,
            branches,
            name: String::new(),
        }
    }

    fn pr(block: u32, port: u32) -> PortRef {
        PortRef {
            block: BlockId(block),
            port,
        }
    }

    #[test]
    fn feedback_runs_below_block_names() {
        let mut sys = System::default();
        sys.blocks.push(blk(1, Rect::new(100.0, 40.0, 150.0, 80.0)));
        sys.blocks.push(blk(2, Rect::new(200.0, 50.0, 230.0, 70.0)));
        // Far below the row: must not push the line down.
        sys.blocks
            .push(blk(3, Rect::new(100.0, 400.0, 150.0, 440.0)));
        let y = feedback_level(&sys, 240.0, 90.0, 70.0, None, None);
        assert_eq!(y, 80.0 + NAME_CLEARANCE + 6.0);
        // Nothing in the span: plain gap below the base.
        assert_eq!(
            feedback_level(&sys, 600.0, 500.0, 70.0, None, None),
            70.0 + FEEDBACK_GAP
        );
        // The endpoint blocks are already covered by `base + FEEDBACK_GAP`;
        // they must not add label clearance on top of it (issue #44 §H-3).
        let mut sys = System::default();
        sys.blocks.push(blk(1, Rect::new(80.0, 5.0, 100.0, 25.0)));
        sys.blocks.push(blk(2, Rect::new(140.0, 0.0, 170.0, 30.0)));
        assert_eq!(
            feedback_level(&sys, 170.0, 80.0, 30.0, Some(BlockId(2)), Some(BlockId(1))),
            50.0
        );

        // An unrelated block sharing an endpoint's x coordinate still needs
        // label clearance; endpoint identity must come from block IDs.
        sys.blocks.push(blk(3, Rect::new(70.0, 10.0, 90.0, 35.0)));
        assert_eq!(
            feedback_level(&sys, 170.0, 80.0, 30.0, Some(BlockId(2)), Some(BlockId(1))),
            59.0
        );
    }

    #[test]
    fn auto_route_cases() {
        let p = |x, y| Point::new(x, y);
        assert_eq!(
            auto_route(p(80.0, 65.0), p(130.0, 65.0), 100.0),
            vec![p(80.0, 65.0), p(130.0, 65.0)]
        );
        assert_eq!(
            auto_route(p(80.0, 65.0), p(130.0, 95.0), 100.0),
            vec![
                p(80.0, 65.0),
                p(105.0, 65.0),
                p(105.0, 95.0),
                p(130.0, 95.0)
            ]
        );
        assert_eq!(
            auto_route(p(80.0, 65.0), p(105.0, 95.0), 100.0),
            vec![
                p(80.0, 65.0),
                p(95.0, 65.0),
                p(95.0, 100.0),
                p(90.0, 100.0),
                p(90.0, 95.0),
                p(105.0, 95.0),
            ]
        );
        assert_eq!(
            auto_route(p(160.0, 65.0), p(130.0, 75.0), 100.0),
            vec![
                p(160.0, 65.0),
                p(175.0, 65.0),
                p(175.0, 100.0),
                p(115.0, 100.0),
                p(115.0, 75.0),
                p(130.0, 75.0),
            ]
        );
    }

    #[test]
    fn tap_positions() {
        let points = [
            Point::new(80.0, 65.0),
            Point::new(105.0, 65.0),
            Point::new(105.0, 95.0),
            Point::new(130.0, 95.0),
        ];
        assert_eq!(
            tap_position(&points, TapPoint { segment: 1, t: 0.5 }),
            Point::new(105.0, 80.0)
        );
        assert_eq!(
            tap_position(&points, TapPoint { segment: 0, t: 0.0 }),
            Point::new(80.0, 65.0)
        );
        assert_eq!(
            tap_position(&points, TapPoint { segment: 9, t: 2.0 }),
            Point::new(130.0, 95.0)
        );
    }

    #[test]
    fn feedback_path_and_branch_tap() {
        let mut sys = System::default();
        sys.blocks
            .push(block(1, "Constant", Rect::new(0.0, 0.0, 30.0, 30.0), &[]));
        sys.blocks.push(block(
            2,
            "Sum",
            Rect::new(80.0, 5.0, 100.0, 25.0),
            &[("Inputs", "+-")],
        ));
        sys.blocks
            .push(block(3, "Gain", Rect::new(140.0, 0.0, 170.0, 30.0), &[]));
        sys.lines = vec![
            line(1, pr(2, 1), Some(pr(3, 1)), vec![]),
            line(2, pr(3, 1), Some(pr(2, 2)), vec![]),
        ];
        let paths = all_paths(&sys);
        assert_eq!(
            paths[1].root.points,
            vec![
                Point::new(170.0, 15.0),
                Point::new(185.0, 15.0),
                Point::new(185.0, 50.0),
                Point::new(65.0, 50.0),
                Point::new(65.0, 20.0),
                Point::new(80.0, 20.0),
            ]
        );
        assert_eq!(paths[1].root.dst, Some(pr(2, 2)));

        sys.blocks
            .push(block(4, "Scope", Rect::new(200.0, 0.0, 230.0, 30.0), &[]));
        sys.lines = vec![line(
            1,
            pr(3, 1),
            Some(pr(4, 1)),
            vec![Branch {
                tap: TapPoint { segment: 0, t: 0.5 },
                points: vec![],
                dst: Some(pr(2, 2)),
                branches: vec![],
            }],
        )];
        let paths = all_paths(&sys);
        assert_eq!(
            paths[0].root.points,
            vec![Point::new(170.0, 15.0), Point::new(200.0, 15.0)]
        );
        assert_eq!(paths[0].root.children.len(), 1);
        assert_eq!(
            paths[0].root.children[0].points,
            vec![
                Point::new(185.0, 15.0),
                Point::new(200.0, 15.0),
                Point::new(200.0, 54.0),
                Point::new(65.0, 54.0),
                Point::new(65.0, 20.0),
                Point::new(80.0, 20.0),
            ]
        );
    }

    #[test]
    fn missing_blocks_and_short_paths() {
        let mut sys = System::default();
        sys.blocks.push(blk(1, Rect::new(0.0, 0.0, 30.0, 30.0)));
        assert!(line_path(&sys, &line(1, pr(99, 1), Some(pr(1, 1)), vec![])).is_none());

        let dangling = line_path(&sys, &line(1, pr(1, 1), Some(pr(99, 1)), vec![])).unwrap();
        assert_eq!(
            dangling.root.points,
            vec![Point::new(30.0, 15.0), Point::new(50.0, 15.0)]
        );
        assert_eq!(dangling.root.dst, None);

        assert_eq!(
            tap_position(&[Point::new(3.0, 4.0)], TapPoint { segment: 0, t: 0.0 }),
            Point::new(3.0, 4.0)
        );
        assert_eq!(
            tap_position(&[], TapPoint { segment: 0, t: 0.0 }),
            Point::default()
        );
    }
}
