use super::tokens;
use super::viewport::Viewport;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl std::fmt::Display for LineStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LineStyle::Solid => "実線",
            LineStyle::Dashed => "破線",
            LineStyle::Dotted => "点線",
        })
    }
}

impl LineStyle {
    pub const ALL: [LineStyle; 3] = [LineStyle::Solid, LineStyle::Dashed, LineStyle::Dotted];

    /// Dash pattern in pixels; empty for a solid line.
    pub fn dashes(self) -> &'static [f32] {
        match self {
            LineStyle::Solid => &[],
            LineStyle::Dashed => &[9.0, 6.0],
            LineStyle::Dotted => &[2.0, 5.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    Root,
    Extremum,
    Intersection,
}

#[derive(Debug, Clone)]
pub struct Marker {
    pub x: f64,
    pub y: f64,
    pub kind: MarkerKind,
    pub color: Rgb,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CurveSpec {
    pub label: String,
    pub color: Rgb,
    pub width: f32,
    pub style: LineStyle,
    pub lines: Vec<Vec<(f64, f64)>>,
    /// Polygon for a definite-integral fill, in math coordinates.
    pub fill: Option<Vec<(f64, f64)>>,
}

/// Colors of everything other than the curves.
#[derive(Debug, Clone, Copy)]
pub struct SceneTheme {
    pub background: Option<Rgb>,
    pub grid: Rgb,
    pub axis: Rgb,
    pub text: Rgb,
    pub legend_background: Rgb,
}

impl SceneTheme {
    /// Export colors; `None` makes the background transparent.
    pub fn light(background: Option<Rgb>) -> Self {
        Self {
            background,
            grid: Rgb(222, 224, 230),
            axis: Rgb(70, 74, 84),
            text: Rgb(50, 54, 64),
            legend_background: Rgb(245, 246, 250),
        }
    }

    /// On-screen dark colors.
    pub fn dark() -> Self {
        Self {
            background: Some(tokens::CANVAS),
            grid: tokens::GRID,
            axis: tokens::AXIS,
            text: tokens::INK_MUTED,
            legend_background: tokens::SURFACE,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlotSpec {
    pub viewport: Viewport,
    pub curves: Vec<CurveSpec>,
    pub markers: Vec<Marker>,
    pub show_grid: bool,
    pub show_ticks: bool,
    pub show_axis_labels: bool,
    pub show_legend: bool,
    pub theme: SceneTheme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

/// A drawing primitive in pixel coordinates, y pointing down.
#[derive(Debug, Clone)]
pub enum Prim {
    Line {
        points: Vec<(f32, f32)>,
        color: Rgb,
        width: f32,
        style: LineStyle,
    },
    Polygon {
        points: Vec<(f32, f32)>,
        color: Rgb,
        opacity: f32,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Rgb,
        opacity: f32,
    },
    Circle {
        x: f32,
        y: f32,
        radius: f32,
        fill: Rgb,
        stroke: Rgb,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        color: Rgb,
        anchor: Anchor,
    },
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub background: Option<Rgb>,
    pub prims: Vec<Prim>,
}

const TICK_SIZE: f32 = 12.0;
const LEGEND_SIZE: f32 = 15.0;
const MARKER_LABEL_SIZE: f32 = 11.5;
const FILL_OPACITY: f32 = 0.28;
const MAX_LEGEND_CHARS: usize = 36;

/// Round tick positions (steps of 1, 2 or 5 times a power of ten) covering `[min, max]`.
pub fn nice_ticks(min: f64, max: f64, target: usize) -> Vec<f64> {
    let span = max - min;
    if span <= 0.0 || !span.is_finite() || target == 0 {
        return Vec::new();
    }
    let rough = span / target as f64;
    let power = 10f64.powf(rough.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * power)
        .find(|s| *s >= rough * (1.0 - 1e-9))
        .unwrap_or(10.0 * power);
    let first = (min / step - 1e-9).ceil() as i64;
    let last = (max / step + 1e-9).floor() as i64;
    (first..=last)
        .map(|k| {
            let v = k as f64 * step;
            if k == 0 { 0.0 } else { v }
        })
        .collect()
}

/// Formats a value without floating-point noise, switching to exponents when very large or small.
pub fn format_tick(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if v.abs() >= 1e6 || v.abs() < 1e-4 {
        let rounded: f64 = format!("{v:.6e}").parse().unwrap_or(v);
        return format!("{rounded:e}");
    }
    let text = format!("{v:.10}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

/// Lays out the whole plot as primitives for a `width` by `height` pixel target.
pub fn build_scene(spec: &PlotSpec, width: f32, height: f32) -> Scene {
    let mut b = Builder {
        spec,
        w: width,
        h: height,
        prims: Vec::new(),
    };
    b.grid_and_axes();
    b.curves();
    b.markers();
    b.legend();
    Scene {
        width,
        height,
        background: spec.theme.background,
        prims: b.prims,
    }
}

struct Builder<'a> {
    spec: &'a PlotSpec,
    w: f32,
    h: f32,
    prims: Vec<Prim>,
}

impl Builder<'_> {
    fn px(&self, x: f64, y: f64) -> (f32, f32) {
        let (px, py) = self.spec.viewport.to_px(x, y, self.w, self.h);
        // Far off-screen points can overflow renderers; the clamp is far beyond any visible slope.
        let limit = 20.0 * self.w.max(self.h);
        (px.clamp(-limit, limit), py.clamp(-limit, limit))
    }

    fn straight(&mut self, a: (f32, f32), b: (f32, f32), color: Rgb, width: f32) {
        self.prims.push(Prim::Line {
            points: vec![a, b],
            color,
            width,
            style: LineStyle::Solid,
        });
    }

    fn grid_and_axes(&mut self) {
        let vp = self.spec.viewport;
        let theme = self.spec.theme;
        let xs = nice_ticks(vp.x_min, vp.x_max, (self.w / 60.0).max(2.0) as usize);
        let ys = nice_ticks(vp.y_min, vp.y_max, (self.h / 45.0).max(2.0) as usize);
        let tick_px_x: Vec<(f64, f32)> = xs.iter().map(|&x| (x, self.px(x, 0.0).0)).collect();
        let tick_px_y: Vec<(f64, f32)> = ys.iter().map(|&y| (y, self.px(0.0, y).1)).collect();

        if self.spec.show_grid {
            // Lines on the very edge would only tint the border pixels.
            let (w, h) = (self.w, self.h);
            for &(_, px) in tick_px_x.iter().filter(|t| t.1 > 1.0 && t.1 < w - 1.0) {
                self.straight((px, 0.0), (px, h), theme.grid, 1.0);
            }
            for &(_, py) in tick_px_y.iter().filter(|t| t.1 > 1.0 && t.1 < h - 1.0) {
                self.straight((0.0, py), (w, py), theme.grid, 1.0);
            }
        }

        let (zero_x, zero_y) = self.px(0.0, 0.0);
        let x_axis_seen = (0.0..=self.h).contains(&zero_y);
        let y_axis_seen = (0.0..=self.w).contains(&zero_x);
        if x_axis_seen {
            self.straight((0.0, zero_y), (self.w, zero_y), theme.axis, 1.4);
        }
        if y_axis_seen {
            self.straight((zero_x, 0.0), (zero_x, self.h), theme.axis, 1.4);
        }

        // Labels stay on screen when the axis itself has scrolled away.
        let label_y = zero_y.clamp(4.0, self.h - TICK_SIZE - 6.0);
        let label_x = zero_x.clamp(30.0, self.w - 6.0);
        if self.spec.show_ticks {
            for (x, px) in tick_px_x {
                if x == 0.0 || px < 8.0 || px > self.w - 8.0 {
                    continue;
                }
                if x_axis_seen {
                    self.straight((px, zero_y - 3.0), (px, zero_y + 3.0), theme.axis, 1.2);
                }
                self.label(
                    px,
                    label_y + TICK_SIZE + 3.0,
                    format_tick(x),
                    Anchor::Middle,
                );
            }
            for (y, py) in tick_px_y {
                if y == 0.0 || py < 8.0 || py > self.h - 8.0 {
                    continue;
                }
                if y_axis_seen {
                    self.straight((zero_x - 3.0, py), (zero_x + 3.0, py), theme.axis, 1.2);
                }
                self.label(
                    label_x - 5.0,
                    py + TICK_SIZE * 0.35,
                    format_tick(y),
                    Anchor::End,
                );
            }
        }
        if self.spec.show_axis_labels {
            self.label(self.w - 8.0, label_y - 6.0, "x".into(), Anchor::End);
            self.label(label_x + 8.0, 14.0, "y".into(), Anchor::Start);
        }
    }

    fn label(&mut self, x: f32, y: f32, text: String, anchor: Anchor) {
        self.prims.push(Prim::Text {
            x,
            y,
            text,
            size: TICK_SIZE,
            color: self.spec.theme.text,
            anchor,
        });
    }

    fn curves(&mut self) {
        for curve in &self.spec.curves {
            if let Some(fill) = &curve.fill {
                let points: Vec<_> = fill.iter().map(|p| self.px(p.0, p.1)).collect();
                self.prims.push(Prim::Polygon {
                    points,
                    color: curve.color,
                    opacity: FILL_OPACITY,
                });
            }
        }
        for curve in &self.spec.curves {
            for line in &curve.lines {
                for points in self.visible_runs(line) {
                    self.prims.push(Prim::Line {
                        points,
                        color: curve.color,
                        width: curve.width,
                        style: curve.style,
                    });
                }
            }
        }
    }

    /// Drops the parts of a polyline that lie entirely off one side of the view.
    fn visible_runs(&self, line: &[(f64, f64)]) -> Vec<Vec<(f32, f32)>> {
        let points: Vec<_> = line.iter().map(|p| self.px(p.0, p.1)).collect();
        let (w, h) = (self.w, self.h);
        let outside_together = |a: (f32, f32), b: (f32, f32)| {
            (a.0 < 0.0 && b.0 < 0.0)
                || (a.0 > w && b.0 > w)
                || (a.1 < 0.0 && b.1 < 0.0)
                || (a.1 > h && b.1 > h)
        };
        let mut runs = Vec::new();
        let mut run: Vec<(f32, f32)> = Vec::new();
        for pair in points.windows(2) {
            if outside_together(pair[0], pair[1]) {
                if run.len() >= 2 {
                    runs.push(std::mem::take(&mut run));
                } else {
                    run.clear();
                }
            } else {
                if run.is_empty() {
                    run.push(pair[0]);
                }
                run.push(pair[1]);
            }
        }
        if run.len() >= 2 {
            runs.push(run);
        }
        runs
    }

    fn markers(&mut self) {
        let mut taken: Vec<(f32, f32, f32, f32)> = Vec::new();
        for marker in &self.spec.markers {
            let (x, y) = self.px(marker.x, marker.y);
            if !(0.0..=self.w).contains(&x) || !(0.0..=self.h).contains(&y) {
                continue;
            }
            let center_fill = self.spec.theme.background.unwrap_or(Rgb(255, 255, 255));
            let (fill, stroke) = match marker.kind {
                MarkerKind::Root => (center_fill, marker.color),
                MarkerKind::Extremum => (marker.color, marker.color),
                MarkerKind::Intersection => (marker.color, self.spec.theme.text),
            };
            self.prims.push(Prim::Circle {
                x,
                y,
                radius: 4.0,
                fill,
                stroke,
            });
            if let Some(text) = &marker.label {
                let right_room = x < self.w - 110.0;
                let (tx, anchor) = if right_room {
                    (x + 7.0, Anchor::Start)
                } else {
                    (x - 7.0, Anchor::End)
                };
                // A label overlapping an earlier one would make both unreadable.
                let width = estimate_text_width(text, MARKER_LABEL_SIZE);
                let left = if right_room { tx } else { tx - width };
                let rect = (left, y - 7.0 - MARKER_LABEL_SIZE, left + width, y - 7.0);
                let overlaps = taken
                    .iter()
                    .any(|t| rect.0 < t.2 && t.0 < rect.2 && rect.1 < t.3 && t.1 < rect.3);
                if overlaps {
                    continue;
                }
                taken.push(rect);
                self.prims.push(Prim::Text {
                    x: tx,
                    y: y - 7.0,
                    text: text.clone(),
                    size: MARKER_LABEL_SIZE,
                    color: self.spec.theme.text,
                    anchor,
                });
            }
        }
    }

    /// A band of swatch + label entries along the top, wrapping across rows.
    fn legend(&mut self) {
        if !self.spec.show_legend || self.spec.curves.is_empty() {
            return;
        }
        const SWATCH: f32 = 26.0;
        const ROW: f32 = 24.0;
        const PAD: f32 = 10.0;
        const GAP: f32 = 20.0;
        let entries: Vec<(&CurveSpec, String, f32)> = self
            .spec
            .curves
            .iter()
            .map(|c| {
                let text = shorten(&c.label);
                let width = SWATCH + 6.0 + estimate_text_width(&text, LEGEND_SIZE);
                (c, text, width)
            })
            .collect();

        let max_row = self.w - 2.0 * PAD - 16.0;
        let mut rows: Vec<Vec<usize>> = vec![Vec::new()];
        let mut used = 0.0;
        for (i, (_, _, width)) in entries.iter().enumerate() {
            let last = rows.len() - 1;
            if !rows[last].is_empty() && used + GAP + width > max_row {
                rows.push(Vec::new());
                used = 0.0;
            }
            let last = rows.len() - 1;
            used += if rows[last].is_empty() {
                *width
            } else {
                GAP + width
            };
            rows[last].push(i);
        }

        let row_width = |row: &Vec<usize>| {
            row.iter().map(|&i| entries[i].2).sum::<f32>()
                + GAP * row.len().saturating_sub(1) as f32
        };
        let box_w = rows.iter().map(row_width).fold(0.0, f32::max) + 2.0 * PAD;
        let box_h = rows.len() as f32 * ROW + 8.0;
        let box_x = 8.0;
        let box_y = 8.0;
        self.prims.push(Prim::Rect {
            x: box_x,
            y: box_y,
            w: box_w,
            h: box_h,
            color: self.spec.theme.legend_background,
            opacity: 0.9,
        });

        let mut placed = Vec::new();
        for (r, row) in rows.iter().enumerate() {
            let center = box_y + 4.0 + ROW * (r as f32 + 0.5);
            let mut x = box_x + PAD;
            for &i in row {
                let (curve, text, width) = &entries[i];
                placed.push(Prim::Line {
                    points: vec![(x, center), (x + SWATCH, center)],
                    color: curve.color,
                    width: curve.width.max(2.0),
                    style: curve.style,
                });
                placed.push(Prim::Text {
                    x: x + SWATCH + 6.0,
                    y: center + LEGEND_SIZE * 0.35,
                    text: text.clone(),
                    size: LEGEND_SIZE,
                    color: self.spec.theme.text,
                    anchor: Anchor::Start,
                });
                x += width + GAP;
            }
        }
        self.prims.extend(placed);
    }
}

fn shorten(label: &str) -> String {
    if label.chars().count() <= MAX_LEGEND_CHARS {
        return label.to_string();
    }
    let kept: String = label.chars().take(MAX_LEGEND_CHARS - 1).collect();
    format!("{kept}…")
}

fn estimate_text_width(text: &str, size: f32) -> f32 {
    text.chars()
        .map(|c| if c.is_ascii() { 0.56 } else { 1.0 })
        .sum::<f32>()
        * size
}
