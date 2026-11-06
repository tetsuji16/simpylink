//! egui layer of the canvas: converts egui input into [`CanvasEvent`]s for the
//! controller and paints the diagram with `egui::Painter` (DES-05 §6).
use super::controller::{CanvasAction, CanvasController, CanvasEvent, CanvasKey, PointerButton};
use super::geometry::{block_ports, chevron_dir};
use super::icons::{paint_frame, paint_icon, subsystem_port_labels};
use super::route::{all_paths, PathNode};
use super::view::{Modifiers, ScreenPos, ScreenVec, ViewTransform};
use crate::theme::diagram_family;
use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2,
};
use sim_core::geom::Point;
use sim_core::ids::BlockId;
use sim_core::model::{Branch, Orientation, PortRef};
use sim_core::model::{Model, System};
use sim_core::path::SystemPath;
use std::collections::BTreeSet;

/// Drag-and-drop payload from the library browser (block_type).
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryDrag(pub String);

/// Selection colour (Simulink blue).
pub const SELECT: Color32 = Color32::from_rgb(0x00, 0x72, 0xBD);
const BLACK: Color32 = Color32::BLACK;

/// The canvas widget for one frame.
pub struct CanvasWidget<'a> {
    pub model: &'a Model,
    pub path: &'a SystemPath,
    pub controller: &'a mut CanvasController,
    /// Last pointer position over the canvas (canvas-local), updated every frame.
    pub hover_pos: &'a mut Option<ScreenPos>,
}

/// egui modifiers → canvas modifiers (`ctrl` = Ctrl, or Cmd on macOS).
pub fn convert_modifiers(m: egui::Modifiers) -> Modifiers {
    Modifiers {
        ctrl: m.command,
        shift: m.shift,
        alt: m.alt,
    }
}

/// egui key → canvas key.
pub fn convert_key(k: egui::Key) -> Option<CanvasKey> {
    use egui::Key as K;
    Some(match k {
        K::Delete => CanvasKey::Delete,
        K::Backspace => CanvasKey::Backspace,
        K::Escape => CanvasKey::Escape,
        K::Space => CanvasKey::Space,
        K::ArrowLeft => CanvasKey::ArrowLeft,
        K::ArrowRight => CanvasKey::ArrowRight,
        K::ArrowUp => CanvasKey::ArrowUp,
        K::ArrowDown => CanvasKey::ArrowDown,
        K::A => CanvasKey::A,
        K::Z => CanvasKey::Z,
        K::Y => CanvasKey::Y,
        K::C => CanvasKey::C,
        K::X => CanvasKey::X,
        K::V => CanvasKey::V,
        K::D => CanvasKey::D,
        K::G => CanvasKey::G,
        K::R => CanvasKey::R,
        K::I => CanvasKey::I,
        K::Plus | K::Equals => CanvasKey::Plus,
        K::Minus => CanvasKey::Minus,
        K::Num0 => CanvasKey::Zero,
        K::Enter => CanvasKey::Enter,
        _ => return None,
    })
}

fn convert_button(b: egui::PointerButton) -> Option<PointerButton> {
    match b {
        egui::PointerButton::Primary => Some(PointerButton::Primary),
        egui::PointerButton::Secondary => Some(PointerButton::Secondary),
        egui::PointerButton::Middle => Some(PointerButton::Middle),
        _ => None,
    }
}

/// Converts one egui event. `Scroll.pos` is (0,0) and must be filled in by the caller.
pub fn convert_event(ev: &egui::Event, origin: Pos2) -> Option<CanvasEvent> {
    let rel = |p: &Pos2| ScreenPos::new(p.x - origin.x, p.y - origin.y);
    match ev {
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers,
        } => {
            let button = convert_button(*button)?;
            let mods = convert_modifiers(*modifiers);
            Some(if *pressed {
                CanvasEvent::PointerDown {
                    pos: rel(pos),
                    button,
                    mods,
                }
            } else {
                CanvasEvent::PointerUp {
                    pos: rel(pos),
                    button,
                    mods,
                }
            })
        }
        egui::Event::PointerMoved(p) => Some(CanvasEvent::PointerMove {
            pos: rel(p),
            mods: Modifiers::default(),
        }),
        egui::Event::PointerGone => Some(CanvasEvent::PointerLeft),
        egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
            ..
        } => {
            let k = match unit {
                egui::MouseWheelUnit::Point => 1.0,
                egui::MouseWheelUnit::Line => 50.0,
                egui::MouseWheelUnit::Page => 800.0,
            };
            Some(CanvasEvent::Scroll {
                pos: ScreenPos::new(0.0, 0.0),
                delta: ScreenVec::new(delta.x * k, delta.y * k),
                mods: convert_modifiers(*modifiers),
            })
        }
        egui::Event::Key {
            key,
            pressed,
            modifiers,
            ..
        } => Some(CanvasEvent::Key {
            key: convert_key(*key)?,
            pressed: *pressed,
            mods: convert_modifiers(*modifiers),
        }),
        egui::Event::Copy => Some(CanvasEvent::Key {
            key: CanvasKey::C,
            pressed: true,
            mods: Modifiers {
                ctrl: true,
                ..Default::default()
            },
        }),
        egui::Event::Cut => Some(CanvasEvent::Key {
            key: CanvasKey::X,
            pressed: true,
            mods: Modifiers {
                ctrl: true,
                ..Default::default()
            },
        }),
        egui::Event::Paste(_) => Some(CanvasEvent::Key {
            key: CanvasKey::V,
            pressed: true,
            mods: Modifiers {
                ctrl: true,
                ..Default::default()
            },
        }),
        _ => None,
    }
}

fn to_pos(origin: Pos2, view: &ViewTransform, p: Point) -> Pos2 {
    let s = view.to_screen(p);
    Pos2::new(origin.x + s.x, origin.y + s.y)
}

impl<'a> CanvasWidget<'a> {
    /// Allocates all available space, handles input and paints. Returns the actions to perform.
    pub fn show(self, ui: &mut egui::Ui) -> Vec<CanvasAction> {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        let origin = rect.min;
        self.controller.viewport = ScreenVec::new(rect.width(), rect.height());
        let mut actions = vec![];
        let hovered = response.hovered();
        let active = hovered || !self.controller.is_idle();
        if response.clicked() || response.drag_started() || response.secondary_clicked() {
            response.request_focus();
        }
        let keys_ok = response.has_focus() || hovered;
        let (events, modifiers, hover) =
            ui.input(|i| (i.events.clone(), i.modifiers, i.pointer.hover_pos()));
        let mods = convert_modifiers(modifiers);
        // Events are ignored while another widget (a text field) has keyboard focus.
        let text_focus = ui.ctx().memory(|m| m.focused()).is_some() && !response.has_focus();
        for ev in &events {
            let Some(mut cev) = convert_event(ev, origin) else {
                continue;
            };
            let skip = match &mut cev {
                CanvasEvent::PointerDown { pos, .. } => {
                    !hovered || !rect.contains(Pos2::new(pos.x + origin.x, pos.y + origin.y))
                }
                CanvasEvent::PointerMove { mods: m, .. } => {
                    *m = mods;
                    !active
                }
                CanvasEvent::PointerUp { .. } | CanvasEvent::PointerLeft => {
                    self.controller.is_idle() && !hovered
                }
                CanvasEvent::Scroll { pos, .. } => {
                    if let Some(h) = hover {
                        *pos = ScreenPos::new(h.x - origin.x, h.y - origin.y);
                    }
                    !hovered
                }
                CanvasEvent::Key { .. } => !keys_ok || text_focus,
                _ => false,
            };
            if skip {
                continue;
            }
            actions.extend(self.controller.handle(cev, self.model, self.path));
        }
        if response.double_clicked() {
            if let Some(h) = hover {
                actions.extend(self.controller.handle(
                    CanvasEvent::DoubleClick {
                        pos: ScreenPos::new(h.x - origin.x, h.y - origin.y),
                        mods,
                    },
                    self.model,
                    self.path,
                ));
            }
        }
        if let Some(p) = response.dnd_release_payload::<LibraryDrag>() {
            if let Some(h) = hover {
                actions.extend(self.controller.handle(
                    CanvasEvent::DropBlock {
                        pos: ScreenPos::new(h.x - origin.x, h.y - origin.y),
                        block_type: p.0.clone(),
                    },
                    self.model,
                    self.path,
                ));
            }
        }
        *self.hover_pos = hover.map(|h| ScreenPos::new(h.x - origin.x, h.y - origin.y));
        if let Some(sys) = self.model.system(self.path) {
            paint(&painter, rect, sys, self.controller);
        }
        actions
    }
}

/// Paints the system (background, lines, blocks, ports, previews).
pub fn paint(painter: &egui::Painter, rect: Rect, sys: &System, c: &CanvasController) {
    let origin = rect.min;
    let view = c.view;
    let z = view.zoom;
    painter.rect_filled(rect, CornerRadius::ZERO, Color32::WHITE);
    let preview = c.preview_with(Some(sys));
    // Apply drag preview to a copy so that lines follow the dragged blocks.
    let moved;
    let sys = if let Some((ids, dx, dy)) = &preview.drag {
        let mut s = sys.clone();
        for b in s.blocks.iter_mut().filter(|b| ids.contains(&b.sid)) {
            b.position = b.position.translated(*dx, *dy);
        }
        moved = s;
        &moved
    } else {
        sys
    };
    let p = |pt: Point| to_pos(origin, &view, pt);
    let lw = z.clamp(1.0, 3.0);
    // Ports that have a line attached (chevrons are drawn only on free ports, as in Simulink).
    let mut used_in: BTreeSet<(BlockId, u32)> = BTreeSet::new();
    let mut used_out: BTreeSet<(BlockId, u32)> = BTreeSet::new();
    for l in &sys.lines {
        used_out.insert((l.src.block, l.src.port));
        collect_dsts(l.dst, &l.branches, &mut used_in);
    }
    for lp in all_paths(sys) {
        let sel = c.selection.lines.contains(&lp.line);
        let stroke = if sel {
            Stroke::new(lw + 1.0, SELECT)
        } else {
            Stroke::new(lw, BLACK)
        };
        paint_node(painter, &p, &lp.root, stroke, z, true);
    }
    let name_font = FontId::new((11.0 * z).clamp(4.0, 60.0), diagram_family());
    for b in &sys.blocks {
        let r = Rect::from_two_pos(
            p(Point::new(b.position.left, b.position.top)),
            p(Point::new(b.position.right, b.position.bottom)),
        );
        let sel = c.selection.blocks.contains(&b.sid);
        let unsupported = sim_core::library::spec(&b.block_type).is_none();
        let fill = if unsupported {
            Color32::from_gray(225)
        } else {
            Color32::WHITE
        };
        if sel {
            painter.rect_filled(
                r.expand(3.0 * z.max(0.7)),
                CornerRadius::same(2),
                Color32::from_rgba_unmultiplied(0x00, 0x72, 0xBD, 40),
            );
        }
        let stroke = Stroke::new(lw, if sel { SELECT } else { BLACK });
        paint_frame(painter, r, b, fill, stroke);
        let ports = block_ports(b);
        let in_screen: Vec<Pos2> = ports.inputs.iter().map(|q| p(*q)).collect();
        paint_icon(painter, r, b, z, &in_screen);
        if b.block_type == "SubSystem" {
            let (ins, outs) = subsystem_port_labels(b);
            let f = FontId::new((9.0 * z).clamp(4.0, 48.0), diagram_family());
            for (q, n) in in_screen.iter().zip(&ins) {
                painter.text(
                    *q + Vec2::new(3.0 * z, 0.0),
                    Align2::LEFT_CENTER,
                    n,
                    f.clone(),
                    BLACK,
                );
            }
            for (q, n) in ports.outputs.iter().zip(&outs) {
                painter.text(
                    p(*q) - Vec2::new(3.0 * z, 0.0),
                    Align2::RIGHT_CENTER,
                    n,
                    f.clone(),
                    BLACK,
                );
            }
        }
        let name_pos = match b.orientation {
            Orientation::Right | Orientation::Left => r.center_bottom() + Vec2::new(0.0, 3.0 * z),
            Orientation::Up | Orientation::Down => r.center_bottom() + Vec2::new(0.0, 3.0 * z),
        };
        painter.text(
            name_pos,
            Align2::CENTER_TOP,
            &b.name,
            name_font.clone(),
            BLACK,
        );
        let chev = Stroke::new(lw, if sel { SELECT } else { BLACK });
        for (i, ip) in ports.inputs.iter().enumerate() {
            if !used_in.contains(&(b.sid, i as u32 + 1)) {
                let tri = chevron_dir(*ip, b.orientation, true).map(&p);
                painter.add(Shape::line(vec![tri[0], tri[2], tri[1]], chev));
            }
        }
        for (i, op) in ports.outputs.iter().enumerate() {
            if !used_out.contains(&(b.sid, i as u32 + 1)) {
                let tri = chevron_dir(*op, b.orientation, false).map(&p);
                painter.add(Shape::line(vec![tri[0], tri[2], tri[1]], chev));
            }
        }
    }
    if let Some(rb) = preview.rubber_band {
        let r = Rect::from_two_pos(
            p(Point::new(rb.left, rb.top)),
            p(Point::new(rb.right, rb.bottom)),
        );
        painter.rect(
            r,
            CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied(0, 0x72, 0xBD, 26),
            Stroke::new(1.0, SELECT),
            StrokeKind::Inside,
        );
    }
    if let Some((a, b)) = preview.drawing_line {
        for s in Shape::dashed_line(&[p(a), p(b)], Stroke::new(1.0, SELECT), 4.0, 3.0) {
            painter.add(s);
        }
    }
}

fn paint_node(
    painter: &egui::Painter,
    p: &dyn Fn(Point) -> Pos2,
    node: &PathNode,
    stroke: Stroke,
    z: f32,
    root: bool,
) {
    // Snap to pixel centres so that 1 px lines stay crisp and black.
    let pts: Vec<Pos2> = node
        .points
        .iter()
        .map(|q| {
            let v = p(*q);
            Pos2::new(
                painter.round_to_pixel_center(v.x),
                painter.round_to_pixel_center(v.y),
            )
        })
        .collect();
    if pts.len() >= 2 {
        painter.add(Shape::line(pts.clone(), stroke));
        if node.dst.is_some() {
            let tip = pts[pts.len() - 1];
            let prev = pts[pts.len() - 2];
            let d = (tip - prev).normalized();
            if d.length() > 0.0 {
                let back = tip - d * 7.0 * z;
                let n = Vec2::new(-d.y, d.x) * 3.5 * z;
                painter.add(Shape::convex_polygon(
                    vec![tip, back + n, back - n],
                    stroke.color,
                    Stroke::NONE,
                ));
            }
        }
        if !root {
            painter.circle_filled(
                pts[0],
                2.5 * z.max(0.6) + (stroke.width - 1.0).max(0.0) * 0.5,
                stroke.color,
            );
        }
    }
    for ch in &node.children {
        paint_node(painter, p, ch, stroke, z, false);
    }
}

fn collect_dsts(dst: Option<PortRef>, branches: &[Branch], out: &mut BTreeSet<(BlockId, u32)>) {
    if let Some(d) = dst {
        out.insert((d.block, d.port));
    }
    for b in branches {
        collect_dsts(b.dst, &b.branches, out);
    }
}
