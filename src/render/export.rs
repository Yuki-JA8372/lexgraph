use super::scene::{Anchor, Prim, Rgb, Scene};
use resvg::{tiny_skia, usvg};
use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("SVG の解析に失敗しました: {0}")]
    Svg(String),
    #[error("{width}x{height} の画像バッファを確保できませんでした")]
    Canvas { width: u32, height: u32 },
    #[error("PNG の生成に失敗しました: {0}")]
    Png(String),
    #[error("PDF の生成に失敗しました: {0}")]
    Pdf(String),
}

/// Straight-alpha RGBA pixels, top row first.
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

fn css(c: Rgb) -> String {
    format!("rgb({},{},{})", c.0, c.1, c.2)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn points_attr(points: &[(f32, f32)]) -> String {
    let mut out = String::with_capacity(points.len() * 12);
    for (x, y) in points {
        let _ = write!(out, "{x:.2},{y:.2} ");
    }
    out
}

/// Writes the scene as a standalone SVG document.
pub fn to_svg(scene: &Scene) -> String {
    let (w, h) = (scene.width, scene.height);
    let mut s = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="Segoe UI, Arial, sans-serif">"#
    );
    s.push('\n');
    if let Some(bg) = scene.background {
        let _ = writeln!(
            s,
            r#"<rect x="0" y="0" width="{w}" height="{h}" fill="{}"/>"#,
            css(bg)
        );
    }
    for prim in &scene.prims {
        let _ = match prim {
            Prim::Line {
                points,
                color,
                width,
                style,
            } => {
                let dash = if style.dashes().is_empty() {
                    String::new()
                } else {
                    let pattern: Vec<String> =
                        style.dashes().iter().map(ToString::to_string).collect();
                    format!(r#" stroke-dasharray="{}""#, pattern.join(" "))
                };
                writeln!(
                    s,
                    r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{width}" stroke-linejoin="round" stroke-linecap="round"{dash}/>"#,
                    points_attr(points),
                    css(*color)
                )
            }
            Prim::Polygon {
                points,
                color,
                opacity,
            } => writeln!(
                s,
                r#"<polygon points="{}" fill="{}" fill-opacity="{opacity}"/>"#,
                points_attr(points),
                css(*color)
            ),
            Prim::Rect {
                x,
                y,
                w,
                h,
                color,
                opacity,
            } => writeln!(
                s,
                r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="5" fill="{}" fill-opacity="{opacity}"/>"#,
                css(*color)
            ),
            Prim::Circle {
                x,
                y,
                radius,
                fill,
                stroke,
            } => writeln!(
                s,
                r#"<circle cx="{x}" cy="{y}" r="{radius}" fill="{}" stroke="{}" stroke-width="1.5"/>"#,
                css(*fill),
                css(*stroke)
            ),
            Prim::Text {
                x,
                y,
                text,
                size,
                color,
                anchor,
            } => {
                let anchor = match anchor {
                    Anchor::Start => "start",
                    Anchor::Middle => "middle",
                    Anchor::End => "end",
                };
                writeln!(
                    s,
                    r#"<text x="{x}" y="{y}" font-size="{size}" fill="{}" text-anchor="{anchor}">{}</text>"#,
                    css(*color),
                    escape(text)
                )
            }
        };
    }
    s.push_str("</svg>\n");
    s
}

/// The system fonts are slow to enumerate, so every export shares one database.
fn raster_fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

fn pdf_fonts() -> Arc<svg2pdf::usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<svg2pdf::usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = svg2pdf::usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// Draws the scene scaled to fill `width` by `height`, so a larger image is
/// sharper rather than showing the same plot with smaller text and thinner lines.
fn rasterize(scene: &Scene, width: u32, height: u32) -> Result<tiny_skia::Pixmap, RenderError> {
    let options = usvg::Options {
        fontdb: raster_fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&to_svg(scene), &options)
        .map_err(|e| RenderError::Svg(e.to_string()))?;
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or(RenderError::Canvas { width, height })?;
    let transform =
        tiny_skia::Transform::from_scale(width as f32 / scene.width, height as f32 / scene.height);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

fn natural_size(scene: &Scene) -> (u32, u32) {
    (scene.width.round() as u32, scene.height.round() as u32)
}

pub fn to_rgba(scene: &Scene) -> Result<RgbaImage, RenderError> {
    let (width, height) = natural_size(scene);
    to_rgba_sized(scene, width, height)
}

pub fn to_rgba_sized(scene: &Scene, width: u32, height: u32) -> Result<RgbaImage, RenderError> {
    let pixmap = rasterize(scene, width, height)?;
    let mut pixels = Vec::with_capacity(pixmap.data().len());
    for p in pixmap.pixels() {
        let c = p.demultiply();
        pixels.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(RgbaImage {
        width: pixmap.width(),
        height: pixmap.height(),
        pixels,
    })
}

pub fn to_png(scene: &Scene) -> Result<Vec<u8>, RenderError> {
    let (width, height) = natural_size(scene);
    to_png_sized(scene, width, height)
}

pub fn to_png_sized(scene: &Scene, width: u32, height: u32) -> Result<Vec<u8>, RenderError> {
    rasterize(scene, width, height)?
        .encode_png()
        .map_err(|e| RenderError::Png(e.to_string()))
}

pub fn to_pdf(scene: &Scene) -> Result<Vec<u8>, RenderError> {
    let options = svg2pdf::usvg::Options {
        fontdb: pdf_fonts(),
        ..Default::default()
    };
    let tree = svg2pdf::usvg::Tree::from_str(&to_svg(scene), &options)
        .map_err(|e| RenderError::Svg(e.to_string()))?;
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|e| RenderError::Pdf(e.to_string()))
}
