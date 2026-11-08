//! Draws a [`Scene`] on an iced canvas frame.
use super::scene::{Anchor, Prim, Rgb, Scene};
use iced::alignment;
use iced::widget::canvas::{self, Frame, LineDash, Path, Stroke, Text};
use iced::{Color, Pixels, Point, Renderer, Size};

pub fn color(c: Rgb) -> Color {
    Color::from_rgb8(c.0, c.1, c.2)
}

fn polyline(points: &[(f32, f32)], close: bool) -> Path {
    Path::new(|b| {
        if let Some((first, rest)) = points.split_first() {
            b.move_to(Point::new(first.0, first.1));
            for p in rest {
                b.line_to(Point::new(p.0, p.1));
            }
            if close {
                b.close();
            }
        }
    })
}

/// Paints the scene's background and primitives, in order.
pub fn draw_scene(frame: &mut Frame<Renderer>, scene: &Scene) {
    if let Some(bg) = scene.background {
        frame.fill_rectangle(
            Point::ORIGIN,
            Size::new(scene.width, scene.height),
            color(bg),
        );
    }
    for prim in &scene.prims {
        match prim {
            Prim::Line {
                points,
                color: c,
                width,
                style,
            } => {
                let stroke = Stroke {
                    style: canvas::Style::Solid(color(*c)),
                    width: *width,
                    line_cap: canvas::LineCap::Round,
                    line_join: canvas::LineJoin::Round,
                    line_dash: LineDash {
                        segments: style.dashes(),
                        offset: 0,
                    },
                };
                frame.stroke(&polyline(points, false), stroke);
            }
            Prim::Polygon {
                points,
                color: c,
                opacity,
            } => {
                frame.fill(&polyline(points, true), color(*c).scale_alpha(*opacity));
            }
            Prim::Rect {
                x,
                y,
                w,
                h,
                color: c,
                opacity,
            } => {
                frame.fill_rectangle(
                    Point::new(*x, *y),
                    Size::new(*w, *h),
                    color(*c).scale_alpha(*opacity),
                );
            }
            Prim::Circle {
                x,
                y,
                radius,
                fill,
                stroke,
            } => {
                let circle = Path::circle(Point::new(*x, *y), *radius);
                frame.fill(&circle, color(*fill));
                frame.stroke(
                    &circle,
                    Stroke::default().with_color(color(*stroke)).with_width(1.5),
                );
            }
            Prim::Text {
                x,
                y,
                text,
                size,
                color: c,
                anchor,
            } => {
                frame.fill_text(label(text, *x, *y, *size, color(*c), *anchor));
            }
        }
    }
}

/// Builds canvas text from a baseline-positioned label.
pub fn label(text: &str, x: f32, y: f32, size: f32, color: Color, anchor: Anchor) -> Text {
    Text {
        content: text.to_string(),
        position: Point::new(x, y - size * 0.8),
        color,
        size: Pixels(size),
        align_x: match anchor {
            Anchor::Start => iced::widget::text::Alignment::Left,
            Anchor::Middle => iced::widget::text::Alignment::Center,
            Anchor::End => iced::widget::text::Alignment::Right,
        },
        align_y: alignment::Vertical::Top,
        max_width: f32::INFINITY,
        shaping: iced::widget::text::Shaping::Advanced,
        ..Text::default()
    }
}
