use serde::{Deserialize, Serialize};

const MIN_SPAN: f64 = 1e-9;
const MAX_SPAN: f64 = 1e12;

/// The visible range of the graph, in math coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x_min: -10.0,
            x_max: 10.0,
            y_min: -10.0,
            y_max: 10.0,
        }
    }
}

impl Viewport {
    pub fn width(&self) -> f64 {
        self.x_max - self.x_min
    }

    pub fn height(&self) -> f64 {
        self.y_max - self.y_min
    }

    /// True if the range is usable: finite, ordered and within the zoom limits.
    pub fn is_valid(&self) -> bool {
        let ok = |span: f64| span.is_finite() && (MIN_SPAN..=MAX_SPAN).contains(&span);
        ok(self.width()) && ok(self.height())
    }

    /// Scales the view around a math-space point, which stays put on screen.
    /// A zoom past the limits leaves the view unchanged.
    pub fn zoom_at(&self, x: f64, y: f64, factor: f64) -> Self {
        let zoomed = Self {
            x_min: x + (self.x_min - x) * factor,
            x_max: x + (self.x_max - x) * factor,
            y_min: y + (self.y_min - y) * factor,
            y_max: y + (self.y_max - y) * factor,
        };
        if zoomed.is_valid() { zoomed } else { *self }
    }

    pub fn panned(&self, dx: f64, dy: f64) -> Self {
        Self {
            x_min: self.x_min + dx,
            x_max: self.x_max + dx,
            y_min: self.y_min + dy,
            y_max: self.y_max + dy,
        }
    }

    pub fn to_px(&self, x: f64, y: f64, w: f32, h: f32) -> (f32, f32) {
        let px = (x - self.x_min) / self.width() * f64::from(w);
        let py = (self.y_max - y) / self.height() * f64::from(h);
        (px as f32, py as f32)
    }

    pub fn from_px(&self, px: f32, py: f32, w: f32, h: f32) -> (f64, f64) {
        let x = self.x_min + f64::from(px) / f64::from(w) * self.width();
        let y = self.y_max - f64::from(py) / f64::from(h) * self.height();
        (x, y)
    }
}
