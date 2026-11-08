//! The interactive graph canvas: wheel zoom, drag pan and hover readout.
use super::Message;
use crate::app::plot::short_number;
use crate::render::canvas::{color, draw_scene, label};
use crate::render::{Anchor, PlotSpec, build_scene};
use iced::widget::canvas::{Action, Cache, Event, Frame, Geometry, Path, Program, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, mouse};

/// What the hover readout shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hover {
    pub x: f64,
    pub y: f64,
    /// Set when the readout snapped to a curve: the index of that curve.
    pub curve: Option<usize>,
}

pub struct Graph<'a> {
    pub spec: &'a PlotSpec,
    pub cache: &'a Cache,
    pub hover: Option<Hover>,
}

#[derive(Default)]
pub struct DragState {
    last: Option<Point>,
    inside: bool,
}

const ZOOM_PER_LINE: f64 = 0.88;
const PIXELS_PER_LINE: f64 = 40.0;

impl Program<Message> for Graph<'_> {
    type State = DragState;

    fn update(
        &self,
        state: &mut DragState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        let Event::Mouse(mouse_event) = event else {
            return None;
        };
        let size = bounds.size();
        let position = cursor.position_in(bounds);
        match mouse_event {
            mouse::Event::WheelScrolled { delta } => {
                let at = position?;
                let lines = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => f64::from(*y),
                    mouse::ScrollDelta::Pixels { y, .. } => f64::from(*y) / PIXELS_PER_LINE,
                };
                let (x, y) = self
                    .spec
                    .viewport
                    .from_px(at.x, at.y, size.width, size.height);
                let factor = ZOOM_PER_LINE.powf(lines);
                Some(Action::publish(Message::Zoom { x, y, factor }).and_capture())
            }
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                let at = position?;
                state.last = Some(at);
                Some(Action::request_redraw().and_capture())
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                state.last.take().map(|_| Action::request_redraw())
            }
            mouse::Event::CursorMoved { .. } => {
                if let Some(last) = state.last {
                    // A drag keeps working when the cursor leaves the canvas.
                    let now = cursor.position()?;
                    let now = Point::new(now.x - bounds.x, now.y - bounds.y);
                    state.last = Some(now);
                    let vp = self.spec.viewport;
                    let dx = -f64::from((now.x - last.x) / size.width) * vp.width();
                    let dy = f64::from((now.y - last.y) / size.height) * vp.height();
                    return Some(Action::publish(Message::Pan { dx, dy }).and_capture());
                }
                match position {
                    Some(at) => {
                        state.inside = true;
                        Some(Action::publish(Message::Hover(Some((at, size)))))
                    }
                    None if state.inside => {
                        state.inside = false;
                        Some(Action::publish(Message::Hover(None)))
                    }
                    None => None,
                }
            }
            mouse::Event::CursorLeft => {
                state.inside = false;
                Some(Action::publish(Message::Hover(None)))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &DragState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let size = bounds.size();
        let base = self.cache.draw(renderer, size, |frame| {
            draw_scene(frame, &build_scene(self.spec, size.width, size.height));
        });
        if self.hover.is_none() && !self.spec.curves.is_empty() {
            return vec![base];
        }
        let mut overlay = Frame::new(renderer, size);
        if self.spec.curves.is_empty() {
            self.draw_empty_hint(&mut overlay);
        }
        if let Some(hover) = self.hover {
            self.draw_hover(&mut overlay, hover, size);
        }
        vec![base, overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &DragState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.last.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

impl Graph<'_> {
    /// Says what the empty graph is waiting for. On screen only, never in exports.
    fn draw_empty_hint(&self, frame: &mut Frame) {
        const HINT: &str = "左の欄に式を入力すると、ここにグラフが表示されます";
        const SIZE: f32 = 15.0;
        // Where the legend goes: the default view has no tick labels here to collide with.
        let box_size = Size::new(HINT.chars().count() as f32 * SIZE + 20.0, 32.0);
        let corner = Point::new(8.0, 8.0);
        frame.fill_rectangle(corner, box_size, color(self.spec.theme.legend_background));
        frame.fill_text(label(
            HINT,
            corner.x + 10.0,
            corner.y + box_size.height / 2.0 + SIZE * 0.35,
            SIZE,
            color(self.spec.theme.text),
            Anchor::Start,
        ));
    }

    /// Marks the snapped point and shows small coordinates in the bottom-right corner.
    fn draw_hover(&self, frame: &mut Frame, hover: Hover, size: Size) {
        let text_color = color(self.spec.theme.text);
        if let Some(curve) = hover.curve.and_then(|i| self.spec.curves.get(i)) {
            let (px, py) = self
                .spec
                .viewport
                .to_px(hover.x, hover.y, size.width, size.height);
            let dot = Path::circle(Point::new(px, py), 5.0);
            frame.fill(&dot, color(curve.color));
            frame.stroke(
                &dot,
                Stroke::default().with_color(text_color).with_width(1.5),
            );
        }
        let content = format!(
            "x = {}   y = {}",
            short_number(hover.x),
            short_number(hover.y)
        );
        const SIZE: f32 = 12.0;
        let box_w = content.chars().count() as f32 * SIZE * 0.56 + 16.0;
        let box_h = 24.0;
        let corner = Point::new(size.width - box_w - 8.0, size.height - box_h - 8.0);
        let background = color(self.spec.theme.legend_background);
        frame.fill_rectangle(
            corner,
            Size::new(box_w, box_h),
            Color {
                a: 0.9,
                ..background
            },
        );
        frame.fill_text(label(
            &content,
            size.width - 16.0,
            size.height - 8.0 - box_h * 0.32,
            SIZE,
            text_color,
            Anchor::End,
        ));
    }
}

pub type GraphCache = Cache;
