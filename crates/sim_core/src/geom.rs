//! Model-space geometry (Simulink pixel units, y grows downwards).
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A point in model coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// Creates a point.
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
    /// Euclidean distance.
    pub fn distance(&self, o: Point) -> f64 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
}

/// Simulink Position = [left, top, right, bottom]. Serialized as a JSON array.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    /// Creates a rectangle (values are kept as given).
    pub fn new(left: f64, top: f64, right: f64, bottom: f64) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }
    /// Rectangle from top-left corner and size.
    pub fn from_min_size(min: Point, w: f64, h: f64) -> Rect {
        Rect::new(min.x, min.y, min.x + w, min.y + h)
    }
    /// Width.
    pub fn width(&self) -> f64 {
        self.right - self.left
    }
    /// Height.
    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }
    /// Center point.
    pub fn center(&self) -> Point {
        Point::new(
            (self.left + self.right) / 2.0,
            (self.top + self.bottom) / 2.0,
        )
    }
    /// True if `p` is inside or on the border.
    pub fn contains(&self, p: Point) -> bool {
        self.left <= p.x && p.x <= self.right && self.top <= p.y && p.y <= self.bottom
    }
    /// Translated copy.
    pub fn translated(&self, dx: f64, dy: f64) -> Rect {
        Rect::new(
            self.left + dx,
            self.top + dy,
            self.right + dx,
            self.bottom + dy,
        )
    }
    /// True if the rectangles overlap or touch.
    pub fn intersects(&self, o: &Rect) -> bool {
        !(o.left > self.right || o.right < self.left || o.top > self.bottom || o.bottom < self.top)
    }
    /// Rectangle spanned by two points.
    pub fn normalized(a: Point, b: Point) -> Rect {
        Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
    }
    /// Smallest rectangle containing both.
    pub fn union(&self, o: &Rect) -> Rect {
        Rect::new(
            self.left.min(o.left),
            self.top.min(o.top),
            self.right.max(o.right),
            self.bottom.max(o.bottom),
        )
    }
}

impl Serialize for Rect {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        [self.left, self.top, self.right, self.bottom].serialize(s)
    }
}

impl<'de> Deserialize<'de> for Rect {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let a = <[f64; 4]>::deserialize(d)?;
        Ok(Rect::new(a[0], a[1], a[2], a[3]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rect_ops() {
        let r = Rect::new(50.0, 50.0, 80.0, 80.0);
        assert_eq!(r.width(), 30.0);
        assert_eq!(r.height(), 30.0);
        assert_eq!(r.center(), Point::new(65.0, 65.0));
        assert!(r.contains(Point::new(50.0, 80.0)));
        assert!(!r.contains(Point::new(49.9, 60.0)));
        assert_eq!(r.translated(10.0, -5.0), Rect::new(60.0, 45.0, 90.0, 75.0));
        assert!(r.intersects(&Rect::new(80.0, 80.0, 90.0, 90.0)));
        assert!(!r.intersects(&Rect::new(81.0, 0.0, 90.0, 10.0)));
        assert_eq!(
            Rect::normalized(Point::new(10.0, 40.0), Point::new(0.0, 5.0)),
            Rect::new(0.0, 5.0, 10.0, 40.0)
        );
        assert_eq!(serde_json::to_string(&r).unwrap(), "[50.0,50.0,80.0,80.0]");
        assert_eq!(
            serde_json::from_str::<Rect>("[1,2,3,4]").unwrap(),
            Rect::new(1.0, 2.0, 3.0, 4.0)
        );
        assert!(serde_json::from_str::<Rect>("[1,2,3]").is_err());
        assert!(serde_json::from_str::<Rect>("{\"left\":1}").is_err());
        assert_eq!(Point::new(0.0, 0.0).distance(Point::new(3.0, 4.0)), 5.0);
    }
}
