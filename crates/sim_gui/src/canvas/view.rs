//! Screen ⇔ model coordinate transform (egui independent).
use sim_core::geom::{Point, Rect};

/// Position in canvas-local logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScreenPos {
    pub x: f32,
    pub y: f32,
}
/// Vector in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScreenVec {
    pub x: f32,
    pub y: f32,
}
/// Keyboard modifiers (ctrl = Command on macOS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl ScreenPos {
    /// New position.
    pub fn new(x: f32, y: f32) -> Self {
        ScreenPos { x, y }
    }
}
impl ScreenVec {
    /// New vector.
    pub fn new(x: f32, y: f32) -> Self {
        ScreenVec { x, y }
    }
    /// Length.
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}
impl std::ops::Sub for ScreenPos {
    type Output = ScreenVec;
    fn sub(self, o: ScreenPos) -> ScreenVec {
        ScreenVec::new(self.x - o.x, self.y - o.y)
    }
}

/// offset = screen position of the model origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    pub offset: ScreenVec,
    pub zoom: f32,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self::identity()
    }
}

impl ViewTransform {
    pub const MIN_ZOOM: f32 = 0.1;
    pub const MAX_ZOOM: f32 = 10.0;
    /// 100 %, origin at the top-left.
    pub fn identity() -> Self {
        ViewTransform {
            offset: ScreenVec::default(),
            zoom: 1.0,
        }
    }
    /// Model → screen.
    pub fn to_screen(&self, p: Point) -> ScreenPos {
        ScreenPos::new(
            self.offset.x + p.x as f32 * self.zoom,
            self.offset.y + p.y as f32 * self.zoom,
        )
    }
    /// Screen → model.
    pub fn to_model(&self, s: ScreenPos) -> Point {
        Point::new(
            ((s.x - self.offset.x) / self.zoom) as f64,
            ((s.y - self.offset.y) / self.zoom) as f64,
        )
    }
    /// Screen delta → model delta.
    pub fn screen_delta_to_model(&self, v: ScreenVec) -> (f64, f64) {
        ((v.x / self.zoom) as f64, (v.y / self.zoom) as f64)
    }
    /// Zoom keeping the model point under `anchor` fixed.
    pub fn zoom_about(&mut self, anchor: ScreenPos, factor: f32) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let m = self.to_model(anchor);
        self.zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        self.offset = ScreenVec::new(
            anchor.x - m.x as f32 * self.zoom,
            anchor.y - m.y as f32 * self.zoom,
        );
    }
    /// Pan by a screen delta.
    pub fn pan(&mut self, v: ScreenVec) {
        self.offset.x += v.x;
        self.offset.y += v.y;
    }
    /// Fits `bounds` into the viewport.
    pub fn fit(&mut self, bounds: Rect, viewport: ScreenVec, margin: f32) {
        let w = bounds.width().max(1.0) as f32;
        let h = bounds.height().max(1.0) as f32;
        let (vw, vh) = (viewport.x - 2.0 * margin, viewport.y - 2.0 * margin);
        self.zoom = if vw <= 0.0 || vh <= 0.0 {
            Self::MIN_ZOOM
        } else {
            (vw / w).min(vh / h).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM)
        };
        let c = bounds.center();
        self.offset = ScreenVec::new(
            viewport.x / 2.0 - c.x as f32 * self.zoom,
            viewport.y / 2.0 - c.y as f32 * self.zoom,
        );
    }
}
