//! Canvas pure-logic tests (Issues #43–#48): view, geometry, route, hit, controller.
use sim_core::edit::Command;
use sim_core::geom::{Point, Rect};
use sim_core::ids::{BlockId, LineId};
use sim_core::json::load_from_path;
use sim_core::model::*;
use sim_core::path::SystemPath;
use sim_gui::canvas::controller::*;
use sim_gui::canvas::geometry::*;
use sim_gui::canvas::hit::*;
use sim_gui::canvas::route::*;
use sim_gui::canvas::view::*;

fn fx(name: &str) -> Model {
    load_from_path(&std::path::PathBuf::from(format!(
        "{}/../../tests/fixtures/models/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    )))
    .unwrap()
}
fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn sp(x: f32, y: f32) -> ScreenPos {
    ScreenPos::new(x, y)
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}
fn m(ctrl: bool, shift: bool, alt: bool) -> Modifiers {
    Modifiers { ctrl, shift, alt }
}
const NONE: Modifiers = Modifiers {
    ctrl: false,
    shift: false,
    alt: false,
};
fn down_b(x: f32, y: f32, b: PointerButton, mods: Modifiers) -> CanvasEvent {
    CanvasEvent::PointerDown {
        pos: sp(x, y),
        button: b,
        mods,
    }
}
fn down(x: f32, y: f32) -> CanvasEvent {
    down_b(x, y, PointerButton::Primary, NONE)
}
fn mv(x: f32, y: f32) -> CanvasEvent {
    CanvasEvent::PointerMove {
        pos: sp(x, y),
        mods: NONE,
    }
}
fn mv_m(x: f32, y: f32, mods: Modifiers) -> CanvasEvent {
    CanvasEvent::PointerMove {
        pos: sp(x, y),
        mods,
    }
}
fn up_b(x: f32, y: f32, b: PointerButton, mods: Modifiers) -> CanvasEvent {
    CanvasEvent::PointerUp {
        pos: sp(x, y),
        button: b,
        mods,
    }
}
fn up(x: f32, y: f32) -> CanvasEvent {
    up_b(x, y, PointerButton::Primary, NONE)
}
fn key(k: CanvasKey, mods: Modifiers) -> CanvasEvent {
    CanvasEvent::Key {
        key: k,
        pressed: true,
        mods,
    }
}
fn feed(c: &mut CanvasController, model: &Model, evs: Vec<CanvasEvent>) -> Vec<CanvasAction> {
    evs.into_iter()
        .flat_map(|e| c.handle(e, model, &SystemPath::root()))
        .collect()
}
fn mvb(blocks: &[u32], dx: f64, dy: f64) -> Vec<CanvasAction> {
    vec![CanvasAction::Execute(Command::MoveBlocks {
        path: SystemPath::root(),
        blocks: blocks.iter().map(|b| BlockId(*b)).collect(),
        dx,
        dy,
    })]
}
fn sel(c: &CanvasController) -> Vec<u32> {
    c.selection.blocks.iter().map(|b| b.0).collect()
}
fn pr(b: u32, p: u32) -> PortRef {
    PortRef {
        block: BlockId(b),
        port: p,
    }
}

#[test]
fn view_transform() {
    let mut v = ViewTransform::identity();
    assert_eq!(v.to_screen(pt(10.0, 20.0)), sp(10.0, 20.0));
    v.zoom_about(sp(100.0, 100.0), 2.0);
    assert_eq!((v.zoom, v.offset), (2.0, ScreenVec::new(-100.0, -100.0)));
    assert_eq!(v.to_model(sp(0.0, 0.0)), pt(50.0, 50.0));
    assert_eq!(
        v.screen_delta_to_model(ScreenVec::new(10.0, 4.0)),
        (5.0, 2.0)
    );
    v.zoom_about(sp(3.0, 3.0), 10.0);
    assert_eq!(v.zoom, 10.0);
    v.zoom_about(sp(3.0, 3.0), 0.0001);
    assert_eq!(v.zoom, 0.1);
    let z = v.zoom;
    v.zoom_about(sp(1.0, 1.0), f32::NAN);
    v.zoom_about(sp(1.0, 1.0), -1.0);
    assert_eq!(v.zoom, z);
    let mut v = ViewTransform::identity();
    v.fit(
        Rect::new(50.0, 49.0, 240.0, 81.0),
        ScreenVec::new(800.0, 600.0),
        20.0,
    );
    assert_eq!(v.zoom, 4.0);
    assert_eq!(v.offset, ScreenVec::new(-180.0, 40.0));
    assert_eq!(v.to_screen(pt(50.0, 49.0)), sp(20.0, 236.0));
    v.fit(
        Rect::new(0.0, 0.0, 0.0, 0.0),
        ScreenVec::new(800.0, 600.0),
        20.0,
    );
    assert_eq!(v.zoom, 10.0);
    v.fit(
        Rect::new(50.0, 49.0, 240.0, 81.0),
        ScreenVec::new(30.0, 30.0),
        20.0,
    );
    assert_eq!(v.zoom, 0.1);
}

#[test]
fn ports_and_bounds() {
    let g = Rect::new(130.0, 50.0, 160.0, 80.0);
    assert_eq!(input_port_pos(g, 1, 1), pt(130.0, 65.0));
    assert_eq!(output_port_pos(g, 1, 1), pt(160.0, 65.0));
    let s = Rect::new(210.0, 49.0, 240.0, 81.0);
    assert!(close(input_port_pos(s, 1, 3).y, 49.0 + 32.0 / 6.0));
    assert!(close(input_port_pos(s, 2, 3).y, 65.0));
    assert_eq!(input_port_pos(g, 3, 2), g.center());
    assert_eq!(
        input_chevron(pt(130.0, 65.0)),
        [pt(124.0, 61.0), pt(124.0, 69.0), pt(130.0, 65.0)]
    );
    assert_eq!(
        system_bounds(&fx("step_gain_scope").root),
        Some(Rect::new(50.0, 49.0, 240.0, 81.0))
    );
    assert_eq!(system_bounds(&System::default()), None);
    assert_eq!(
        distance_to_segment(pt(105.0, 70.0), pt(80.0, 65.0), pt(130.0, 65.0)),
        (5.0, 0.5)
    );
    assert_eq!(
        distance_to_segment(pt(70.0, 65.0), pt(80.0, 65.0), pt(130.0, 65.0)),
        (10.0, 0.0)
    );
}

#[test]
fn routing() {
    assert_eq!(
        auto_route(pt(80.0, 65.0), pt(130.0, 65.0), 100.0),
        vec![pt(80.0, 65.0), pt(130.0, 65.0)]
    );
    assert_eq!(
        auto_route(pt(80.0, 65.0), pt(130.0, 95.0), 100.0),
        vec![
            pt(80.0, 65.0),
            pt(105.0, 65.0),
            pt(105.0, 95.0),
            pt(130.0, 95.0)
        ]
    );
    assert_eq!(
        auto_route(pt(80.0, 65.0), pt(105.0, 95.0), 100.0),
        vec![
            pt(80.0, 65.0),
            pt(95.0, 65.0),
            pt(95.0, 100.0),
            pt(90.0, 100.0),
            pt(90.0, 95.0),
            pt(105.0, 95.0)
        ]
    );
    assert_eq!(
        auto_route(pt(160.0, 65.0), pt(130.0, 75.0), 100.0),
        vec![
            pt(160.0, 65.0),
            pt(175.0, 65.0),
            pt(175.0, 100.0),
            pt(115.0, 100.0),
            pt(115.0, 75.0),
            pt(130.0, 75.0)
        ]
    );
    let pts = [
        pt(80.0, 65.0),
        pt(105.0, 65.0),
        pt(105.0, 95.0),
        pt(130.0, 95.0),
    ];
    assert_eq!(
        tap_position(&pts, TapPoint { segment: 1, t: 0.5 }),
        pt(105.0, 80.0)
    );
    assert_eq!(
        tap_position(&pts, TapPoint { segment: 9, t: 2.0 }),
        pt(130.0, 95.0)
    );
    let paths = all_paths(&fx("step_gain_scope").root);
    assert_eq!(paths[0].root.points, vec![pt(80.0, 65.0), pt(130.0, 65.0)]);
    assert_eq!(paths[0].root.dst, Some(pr(2, 1)));
}

#[test]
fn hit_testing() {
    let model = fx("step_gain_scope");
    let s = &model.root;
    let v = ViewTransform::identity();
    let h = |x, y| hit_test(s, &v, sp(x, y));
    assert_eq!(h(165.0, 65.0), HitTarget::OutputPort(pr(2, 1)));
    assert_eq!(h(128.0, 65.0), HitTarget::InputPort(pr(2, 1)));
    assert_eq!(h(158.0, 65.0), HitTarget::OutputPort(pr(2, 1)));
    assert_eq!(h(145.0, 65.0), HitTarget::Block(BlockId(2)));
    assert_eq!(
        h(105.0, 65.0),
        HitTarget::Line {
            line: LineId(1),
            branch: vec![],
            segment: 0,
            t: 0.5
        }
    );
    assert_eq!(
        h(105.0, 69.0),
        HitTarget::Line {
            line: LineId(1),
            branch: vec![],
            segment: 0,
            t: 0.5
        }
    );
    assert_eq!(h(105.0, 71.0), HitTarget::Empty);
    let mut z = ViewTransform::identity();
    z.zoom = 2.0;
    assert_eq!(
        hit_test(s, &z, sp(290.0, 130.0)),
        HitTarget::Block(BlockId(2))
    );
    match hit_test(s, &z, sp(330.0, 130.0)) {
        HitTarget::Line { line, t, .. } => assert!(line == LineId(2) && close(t, 0.1)),
        o => panic!("{o:?}"),
    }
    assert_eq!(
        hit_test(s, &z, sp(326.0, 130.0)),
        HitTarget::OutputPort(pr(2, 1))
    );
    assert_eq!(
        blocks_in_rect(s, Rect::new(70.0, 40.0, 135.0, 90.0)),
        vec![BlockId(1), BlockId(2)]
    );
    assert_eq!(
        hit_test(&System::default(), &v, sp(1.0, 1.0)),
        HitTarget::Empty
    );
}

#[test]
fn select_and_drag() {
    let model = fx("step_gain_scope");
    let new = CanvasController::new;
    let mut c = new();
    assert!(feed(&mut c, &model, vec![down(145.0, 65.0), up(145.0, 65.0)]).is_empty());
    assert_eq!(sel(&c), vec![2]);
    feed(&mut c, &model, vec![down(300.0, 300.0), up(300.0, 300.0)]);
    assert!(sel(&c).is_empty());
    feed(
        &mut c,
        &model,
        vec![
            down(145.0, 65.0),
            up(145.0, 65.0),
            down_b(65.0, 65.0, PointerButton::Primary, m(false, true, false)),
            up(65.0, 65.0),
        ],
    );
    assert_eq!(sel(&c), vec![1, 2]);
    feed(
        &mut c,
        &model,
        vec![
            down_b(65.0, 65.0, PointerButton::Primary, m(false, true, false)),
            up(65.0, 65.0),
        ],
    );
    assert_eq!(sel(&c), vec![2]);
    let mut c = new();
    feed(&mut c, &model, vec![down(145.0, 65.0), mv(147.0, 65.0)]);
    assert_eq!(c.preview().drag, None);
    feed(&mut c, &model, vec![mv(165.0, 75.0)]);
    assert_eq!(c.preview().drag, Some((vec![BlockId(2)], 20.0, 10.0)));
    assert_eq!(
        feed(&mut c, &model, vec![up(165.0, 75.0)]),
        mvb(&[2], 20.0, 10.0)
    );
    assert_eq!(c.preview(), CanvasPreview::default());
    let mut c = new();
    c.view.zoom = 2.0;
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![down(290.0, 130.0), mv(330.0, 150.0), up(330.0, 150.0)]
        ),
        mvb(&[2], 20.0, 10.0)
    );
    let mut c = new();
    feed(
        &mut c,
        &model,
        vec![
            down(145.0, 65.0),
            up(145.0, 65.0),
            down_b(65.0, 65.0, PointerButton::Primary, m(false, true, false)),
            up(65.0, 65.0),
        ],
    );
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![down(65.0, 65.0), mv(85.0, 65.0), up(85.0, 65.0)]
        ),
        mvb(&[1, 2], 20.0, 0.0)
    );
    let mut c = new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down(145.0, 65.0),
                mv(150.0, 66.0),
                mv_m(175.0, 70.0, m(false, true, false)),
                up(175.0, 70.0)
            ]
        ),
        mvb(&[2], 30.0, 0.0)
    );
    let mut c = new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![down(145.0, 65.0), mv(165.6, 75.4), up(165.6, 75.4)]
        ),
        mvb(&[2], 21.0, 10.0)
    );
    let mut c = new();
    feed(&mut c, &model, vec![down(70.0, 40.0), mv(135.0, 90.0)]);
    assert_eq!(
        c.preview().rubber_band,
        Some(Rect::new(70.0, 40.0, 135.0, 90.0))
    );
    feed(&mut c, &model, vec![up(135.0, 90.0)]);
    assert_eq!(sel(&c), vec![1, 2]);
    let mut c = new();
    assert!(feed(
        &mut c,
        &model,
        vec![
            down(145.0, 65.0),
            mv(165.0, 75.0),
            key(CanvasKey::Escape, NONE)
        ]
    )
    .is_empty());
    assert!(c.is_idle() && c.preview() == CanvasPreview::default());
    assert!(feed(&mut c, &model, vec![up(165.0, 75.0)]).is_empty());
    let mut c = new();
    let dup = vec![CanvasAction::DuplicateBlocks {
        blocks: vec![BlockId(2)],
        dx: 20.0,
        dy: 10.0,
    }];
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down_b(145.0, 65.0, PointerButton::Primary, m(true, false, false)),
                mv(165.0, 75.0),
                up(165.0, 75.0)
            ]
        ),
        dup
    );
    let mut c = new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down_b(145.0, 65.0, PointerButton::Secondary, NONE),
                mv(165.0, 75.0),
                up_b(165.0, 75.0, PointerButton::Secondary, NONE)
            ]
        ),
        dup
    );
    let mut c = new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![CanvasEvent::DoubleClick {
                pos: sp(145.0, 65.0),
                mods: NONE
            }]
        ),
        vec![CanvasAction::OpenBlockDialog(BlockId(2))]
    );
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![CanvasEvent::DoubleClick {
                pos: sp(300.0, 300.0),
                mods: NONE
            }]
        ),
        vec![CanvasAction::QuickInsert {
            at: pt(300.0, 300.0)
        }]
    );
    let mut c = new();
    feed(&mut c, &model, vec![down(105.0, 65.0), up(105.0, 65.0)]);
    assert_eq!(
        c.selection.lines.iter().map(|l| l.0).collect::<Vec<_>>(),
        vec![1]
    );
    assert!(c.selection.blocks.is_empty());
    let mut c = new();
    assert!(feed(
        &mut c,
        &model,
        vec![
            down(145.0, 65.0),
            mv(146.0, 65.0),
            mv(145.0, 65.0),
            up(145.0, 65.0)
        ]
    )
    .is_empty());
    assert!(c
        .handle(up(1.0, 1.0), &model, &SystemPath(vec![BlockId(99)]))
        .is_empty());
}

fn m1() -> Model {
    let mut m = fx("step_gain_scope");
    m.root.lines.retain(|l| l.id != LineId(2));
    m
}
fn add_line(src: PortRef, dst: PortRef) -> Vec<CanvasAction> {
    vec![CanvasAction::Execute(Command::AddLine {
        path: SystemPath::root(),
        line: Line {
            id: LineId(3),
            src,
            points: vec![],
            dst: Some(dst),
            branches: vec![],
            name: String::new(),
        },
    })]
}

#[test]
fn connecting_lines() {
    let model = m1();
    let new = CanvasController::new;
    let want = add_line(pr(2, 1), pr(3, 1));
    assert_eq!(
        feed(
            &mut new(),
            &model,
            vec![down(161.0, 65.0), mv(180.0, 65.0), up(210.0, 65.0)]
        ),
        want
    );
    assert_eq!(
        feed(
            &mut new(),
            &model,
            vec![down(161.0, 65.0), mv(180.0, 65.0), up(225.0, 65.0)]
        ),
        want
    );
    assert!(feed(
        &mut new(),
        &model,
        vec![down(161.0, 65.0), mv(180.0, 65.0), up(300.0, 300.0)]
    )
    .is_empty());
    assert_eq!(
        feed(
            &mut new(),
            &model,
            vec![down(81.0, 65.0), mv(100.0, 70.0), up(129.0, 65.0)]
        ),
        vec![CanvasAction::Status("Input port already connected".into())]
    );
    assert_eq!(
        feed(
            &mut new(),
            &model,
            vec![down(209.0, 65.0), mv(190.0, 65.0), up(161.0, 65.0)]
        ),
        want
    );
    let mut l1 = model.root.lines[0].clone();
    l1.branches.push(Branch {
        tap: TapPoint { segment: 0, t: 0.5 },
        points: vec![],
        dst: Some(pr(3, 1)),
        branches: vec![],
    });
    let br = vec![CanvasAction::Execute(Command::ReplaceLine {
        path: SystemPath::root(),
        line: l1,
    })];
    assert_eq!(
        feed(
            &mut new(),
            &model,
            vec![
                down_b(105.0, 65.0, PointerButton::Primary, m(true, false, false)),
                mv(110.0, 90.0),
                up(225.0, 65.0)
            ]
        ),
        br
    );
    assert_eq!(
        feed(
            &mut new(),
            &model,
            vec![
                down_b(105.0, 65.0, PointerButton::Secondary, NONE),
                mv(110.0, 90.0),
                up_b(225.0, 65.0, PointerButton::Secondary, NONE)
            ]
        ),
        br
    );
    assert!(feed(
        &mut new(),
        &model,
        vec![down(105.0, 65.0), mv(110.0, 90.0), up(225.0, 65.0)]
    )
    .is_empty());
    let mut c = new();
    let ctrl = m(true, false, false);
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down(145.0, 65.0),
                up(145.0, 65.0),
                down_b(225.0, 65.0, PointerButton::Primary, ctrl),
                up_b(225.0, 65.0, PointerButton::Primary, ctrl)
            ]
        ),
        want
    );
    assert_eq!(sel(&c), vec![3]);
    let mut c = new();
    assert!(feed(
        &mut c,
        &model,
        vec![
            down_b(225.0, 65.0, PointerButton::Primary, ctrl),
            up_b(225.0, 65.0, PointerButton::Primary, ctrl)
        ]
    )
    .is_empty());
    assert_eq!(sel(&c), vec![3]);
    let mut c = new();
    feed(&mut c, &model, vec![down(161.0, 65.0), mv(180.0, 65.0)]);
    assert_eq!(
        c.preview_with(Some(&model.root)).drawing_line,
        Some((pt(160.0, 65.0), pt(180.0, 65.0)))
    );
    assert!(feed(
        &mut c,
        &model,
        vec![key(CanvasKey::Escape, NONE), up(210.0, 65.0)]
    )
    .is_empty());
    let mut c = new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down(65.0, 65.0),
                up(65.0, 65.0),
                down_b(145.0, 65.0, PointerButton::Primary, ctrl),
                up_b(145.0, 65.0, PointerButton::Primary, ctrl)
            ]
        ),
        vec![CanvasAction::Status("No free input port on 'Gain'".into())]
    );
    let mut c = new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![
                down(225.0, 65.0),
                up(225.0, 65.0),
                down_b(145.0, 65.0, PointerButton::Primary, ctrl),
                up_b(145.0, 65.0, PointerButton::Primary, ctrl)
            ]
        ),
        vec![CanvasAction::Status(
            "Block 'Scope' has no output port".into()
        )]
    );
}

#[test]
fn keys_zoom_pan() {
    let model = fx("step_gain_scope");
    let new = || {
        let mut c = CanvasController::new();
        c.viewport = ScreenVec::new(800.0, 600.0);
        c
    };
    let mut c = new();
    feed(&mut c, &model, vec![down(145.0, 65.0), up(145.0, 65.0)]);
    assert_eq!(
        feed(&mut c, &model, vec![key(CanvasKey::Delete, NONE)]),
        vec![CanvasAction::Execute(Command::DeleteItems {
            path: SystemPath::root(),
            blocks: vec![BlockId(2)],
            lines: vec![]
        })]
    );
    assert!(c.selection.is_empty());
    let mut c = new();
    feed(&mut c, &model, vec![down(105.0, 65.0), up(105.0, 65.0)]);
    assert_eq!(
        feed(&mut c, &model, vec![key(CanvasKey::Backspace, NONE)]),
        vec![CanvasAction::Execute(Command::DeleteItems {
            path: SystemPath::root(),
            blocks: vec![],
            lines: vec![LineId(1)]
        })]
    );
    assert!(feed(&mut new(), &model, vec![key(CanvasKey::Delete, NONE)]).is_empty());
    assert_eq!(
        feed(
            &mut new(),
            &model,
            vec![key(CanvasKey::Z, m(true, false, false))]
        ),
        vec![CanvasAction::Undo]
    );
    assert_eq!(
        feed(
            &mut new(),
            &model,
            vec![key(CanvasKey::Y, m(true, false, false))]
        ),
        vec![CanvasAction::Redo]
    );
    assert_eq!(
        feed(
            &mut new(),
            &model,
            vec![key(CanvasKey::Z, m(true, true, false))]
        ),
        vec![CanvasAction::Redo]
    );
    let mut c = new();
    feed(
        &mut c,
        &model,
        vec![key(CanvasKey::A, m(true, false, false))],
    );
    assert_eq!((sel(&c), c.selection.lines.len()), (vec![1, 2, 3], 2));
    feed(
        &mut c,
        &model,
        vec![key(CanvasKey::A, m(true, false, true))],
    );
    assert!(c.selection.is_empty());
    let mut c = new();
    feed(&mut c, &model, vec![down(145.0, 65.0), up(145.0, 65.0)]);
    assert_eq!(
        feed(&mut c, &model, vec![key(CanvasKey::ArrowRight, NONE)]),
        mvb(&[2], 1.0, 0.0)
    );
    assert_eq!(
        feed(&mut c, &model, vec![key(CanvasKey::ArrowUp, NONE)]),
        mvb(&[2], 0.0, -1.0)
    );
    let mut c = new();
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![mv(100.0, 100.0), key(CanvasKey::V, m(true, false, false))]
        ),
        vec![CanvasAction::Paste {
            at: pt(100.0, 100.0)
        }]
    );
    let mut c = new();
    feed(
        &mut c,
        &model,
        vec![CanvasEvent::Scroll {
            pos: sp(100.0, 100.0),
            delta: ScreenVec::new(0.0, 100.0),
            mods: NONE,
        }],
    );
    assert!((c.view.zoom - 1.0015f32.powf(100.0)).abs() < 1e-4);
    assert!(c.view.to_model(sp(100.0, 100.0)).distance(pt(100.0, 100.0)) < 1e-3);
    let mut c = new();
    feed(
        &mut c,
        &model,
        vec![CanvasEvent::Scroll {
            pos: sp(1.0, 1.0),
            delta: ScreenVec::new(0.0, 50.0),
            mods: m(true, false, false),
        }],
    );
    assert_eq!(
        (c.view.offset, c.view.zoom),
        (ScreenVec::new(0.0, 50.0), 1.0)
    );
    let mut c = new();
    feed(
        &mut c,
        &model,
        vec![CanvasEvent::Scroll {
            pos: sp(1.0, 1.0),
            delta: ScreenVec::new(0.0, 50.0),
            mods: m(false, true, false),
        }],
    );
    assert_eq!(c.view.offset, ScreenVec::new(50.0, 0.0));
    let mut c = new();
    feed(
        &mut c,
        &model,
        vec![
            down_b(100.0, 100.0, PointerButton::Middle, NONE),
            mv(130.0, 90.0),
            up_b(130.0, 90.0, PointerButton::Middle, NONE),
        ],
    );
    assert_eq!(c.view.offset, ScreenVec::new(30.0, -10.0));
    let mut c = new();
    let space_up = CanvasEvent::Key {
        key: CanvasKey::Space,
        pressed: false,
        mods: NONE,
    };
    assert_eq!(
        feed(
            &mut c,
            &model,
            vec![key(CanvasKey::Space, NONE), space_up.clone()]
        ),
        vec![CanvasAction::FitToView]
    );
    assert_eq!(
        (c.view.zoom, c.view.offset),
        (4.0, ScreenVec::new(-180.0, 40.0))
    );
    let mut c = new();
    assert!(feed(
        &mut c,
        &model,
        vec![
            key(CanvasKey::Space, NONE),
            down(100.0, 100.0),
            mv(150.0, 100.0),
            up(150.0, 100.0),
            space_up
        ]
    )
    .is_empty());
    assert_eq!(
        (c.view.offset, c.view.zoom),
        (ScreenVec::new(50.0, 0.0), 1.0)
    );
    let mut c = new();
    feed(
        &mut c,
        &model,
        vec![key(CanvasKey::Plus, m(true, false, false))],
    );
    assert_eq!(c.view.zoom, 1.25);
    assert!(c.view.to_model(sp(400.0, 300.0)).distance(pt(400.0, 300.0)) < 1e-3);
    feed(
        &mut c,
        &model,
        vec![key(CanvasKey::Zero, m(true, false, false))],
    );
    assert!((c.view.zoom - 1.0).abs() < 1e-6);
    let mut c = new();
    assert!(feed(
        &mut c,
        &model,
        vec![
            down(145.0, 65.0),
            mv(165.0, 75.0),
            key(CanvasKey::Delete, NONE)
        ]
    )
    .is_empty());
    assert!(!c.is_idle());
    let mut c = new();
    c.fit_to_view(&System::default());
    assert_eq!(c.view, ViewTransform::identity());
    let mut c = new();
    let r = feed(
        &mut c,
        &model,
        vec![CanvasEvent::DropBlock {
            pos: sp(100.0, 100.0),
            block_type: "Constant".into(),
        }],
    );
    let CanvasAction::Execute(Command::AddBlock { block, .. }) = &r[0] else {
        panic!("{r:?}")
    };
    assert_eq!(
        (block.sid, block.name.as_str(), block.position),
        (BlockId(4), "Constant", Rect::new(85.0, 85.0, 115.0, 115.0))
    );
    assert_eq!(sel(&c), vec![4]);
    let r = feed(
        &mut c,
        &model,
        vec![CanvasEvent::DropBlock {
            pos: sp(1.0, 1.0),
            block_type: "Nope".into(),
        }],
    );
    assert_eq!(
        r,
        vec![CanvasAction::Status("unknown block type 'Nope'".into())]
    );
    let mut c = new();
    feed(&mut c, &model, vec![down(145.0, 65.0), up(145.0, 65.0)]);
    let r = feed(
        &mut c,
        &model,
        vec![key(CanvasKey::R, m(true, false, false))],
    );
    assert!(
        matches!(&r[0], CanvasAction::Execute(Command::Batch { commands, .. }) if commands == &vec![Command::SetOrientation { path: SystemPath::root(), block: BlockId(2), orientation: Orientation::Down }])
    );
}
