//! Port positions and shapes (egui independent).
use sim_core::geom::{Point, Rect};
use sim_core::library::port_counts;
use sim_core::model::{Block, Orientation, System};

pub const PORT_HIT_RADIUS_PX: f32 = 8.0;
pub const LINE_HIT_DISTANCE_PX: f32 = 5.0;
pub const CHEVRON_W: f64 = 6.0;
pub const CHEVRON_H: f64 = 8.0;

fn frac(port: u32, n: u32) -> Option<f64> {
    if n == 0 || port == 0 || port > n {
        None
    } else {
        Some((2 * port - 1) as f64 / (2 * n) as f64)
    }
}

/// Input port position for an orientation.
pub fn input_port_pos_o(r: Rect, port: u32, n: u32, o: Orientation) -> Point {
    let Some(f) = frac(port, n) else {
        return r.center();
    };
    match o {
        Orientation::Right => Point::new(r.left, r.top + r.height() * f),
        Orientation::Left => Point::new(r.right, r.top + r.height() * f),
        Orientation::Down => Point::new(r.right - r.width() * f, r.top),
        Orientation::Up => Point::new(r.left + r.width() * f, r.bottom),
    }
}

/// Output port position for an orientation.
pub fn output_port_pos_o(r: Rect, port: u32, n: u32, o: Orientation) -> Point {
    let Some(f) = frac(port, n) else {
        return r.center();
    };
    match o {
        Orientation::Right => Point::new(r.right, r.top + r.height() * f),
        Orientation::Left => Point::new(r.left, r.top + r.height() * f),
        Orientation::Down => Point::new(r.right - r.width() * f, r.bottom),
        Orientation::Up => Point::new(r.left + r.width() * f, r.top),
    }
}

/// Input port position (orientation Right).
pub fn input_port_pos(r: Rect, port: u32, n: u32) -> Point {
    input_port_pos_o(r, port, n, Orientation::Right)
}
/// Output port position (orientation Right).
pub fn output_port_pos(r: Rect, port: u32, n: u32) -> Point {
    output_port_pos_o(r, port, n, Orientation::Right)
}

/// Unit direction in which signals leave an output port.
pub fn out_dir(o: Orientation) -> (f64, f64) {
    match o {
        Orientation::Right => (1.0, 0.0),
        Orientation::Left => (-1.0, 0.0),
        Orientation::Down => (0.0, 1.0),
        Orientation::Up => (0.0, -1.0),
    }
}

/// All port positions of a block.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BlockPorts {
    pub inputs: Vec<Point>,
    pub outputs: Vec<Point>,
}

/// Port positions (empty if the block's port counts are invalid).
pub fn block_ports(block: &Block) -> BlockPorts {
    block_ports_at(block, block.position)
}

/// Port positions for the block drawn at `r`.
pub fn block_ports_at(block: &Block, r: Rect) -> BlockPorts {
    let Ok(c) = port_counts(block) else {
        return BlockPorts::default();
    };
    BlockPorts {
        inputs: (1..=c.inputs)
            .map(|p| input_port_pos_o(r, p, c.inputs, block.orientation))
            .collect(),
        outputs: (1..=c.outputs)
            .map(|p| output_port_pos_o(r, p, c.outputs, block.orientation))
            .collect(),
    }
}

fn chevron(tip: Point, dir: (f64, f64)) -> [Point; 3] {
    let (dx, dy) = dir;
    let (px, py) = (-dy, dx);
    let back = Point::new(tip.x - dx * CHEVRON_W, tip.y - dy * CHEVRON_W);
    [
        Point::new(back.x + px * CHEVRON_H / 2.0, back.y + py * CHEVRON_H / 2.0),
        Point::new(back.x - px * CHEVRON_H / 2.0, back.y - py * CHEVRON_H / 2.0),
        tip,
    ]
}

/// Input chevron for orientation Right: [(x-6,y-4),(x-6,y+4),(x,y)].
pub fn input_chevron(p: Point) -> [Point; 3] {
    let c = chevron(p, (1.0, 0.0));
    [
        Point::new(c[1].x, c[1].y.min(c[0].y)),
        Point::new(c[0].x, c[1].y.max(c[0].y)),
        c[2],
    ]
}
/// Output chevron for orientation Right: [(x,y-4),(x,y+4),(x+6,y)].
pub fn output_chevron(p: Point) -> [Point; 3] {
    [
        Point::new(p.x, p.y - 4.0),
        Point::new(p.x, p.y + 4.0),
        Point::new(p.x + 6.0, p.y),
    ]
}
/// Chevron for any orientation (pointing in the signal direction).
pub fn chevron_dir(port: Point, o: Orientation, is_input: bool) -> [Point; 3] {
    let d = out_dir(o);
    if is_input {
        chevron(port, d)
    } else {
        chevron(
            Point::new(port.x + d.0 * CHEVRON_W, port.y + d.1 * CHEVRON_W),
            d,
        )
    }
}

/// Union of all block rectangles.
pub fn system_bounds(sys: &System) -> Option<Rect> {
    sys.blocks
        .iter()
        .map(|b| b.position)
        .reduce(|a, b| a.union(&b))
}

/// Distance from `p` to segment ab and the projection parameter t.
pub fn distance_to_segment(p: Point, a: Point, b: Point) -> (f64, f64) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return (p.distance(a), 0.0);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    (p.distance(Point::new(a.x + t * dx, a.y + t * dy)), t)
}
