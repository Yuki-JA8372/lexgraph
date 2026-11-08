//! Image export and small conversions for the user interface.
use super::model::{Document, PngSize};
use super::plot::{Compiled, build_plot};
use crate::core::ParseError;
use crate::render::{
    RenderError, Rgb, RgbaImage, Scene, SceneTheme, build_scene, to_pdf, to_png_sized,
    to_rgba_sized, to_svg,
};
use std::path::Path;

const EXPORT_WIDTH: f32 = 1600.0;
const EXPORT_HEIGHT: f32 = 1000.0;
const EXPORT_SAMPLES: usize = 1600;
const WHITE: Rgb = Rgb(255, 255, 255);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Svg,
    Pdf,
}

impl ExportFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "png" => Some(Self::Png),
            "svg" => Some(Self::Svg),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }
}

fn export_scene(doc: &Document, compiled: &Compiled, transparent: bool) -> Scene {
    let background = if transparent { None } else { Some(WHITE) };
    let theme = SceneTheme::light(background);
    let plot = build_plot(doc, compiled, theme, EXPORT_SAMPLES);
    build_scene(&plot.spec, EXPORT_WIDTH, EXPORT_HEIGHT)
}

/// Renders the graph to file contents, with a white or transparent background.
pub fn export_bytes(
    doc: &Document,
    compiled: &Compiled,
    format: ExportFormat,
    transparent: bool,
    png_size: PngSize,
) -> Result<Vec<u8>, RenderError> {
    let scene = export_scene(doc, compiled, transparent);
    let (width, height) = png_size.pixels();
    match format {
        ExportFormat::Png => to_png_sized(&scene, width, height),
        ExportFormat::Svg => Ok(to_svg(&scene).into_bytes()),
        ExportFormat::Pdf => to_pdf(&scene),
    }
}

/// Renders the graph to pixels for the clipboard.
pub fn export_rgba(
    doc: &Document,
    compiled: &Compiled,
    transparent: bool,
    png_size: PngSize,
) -> Result<RgbaImage, RenderError> {
    let (width, height) = png_size.pixels();
    to_rgba_sized(&export_scene(doc, compiled, transparent), width, height)
}

pub fn parse_hex_color(text: &str) -> Option<Rgb> {
    let hex = text.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |range| u8::from_str_radix(&hex[range], 16).ok();
    Some(Rgb(channel(0..2)?, channel(2..4)?, channel(4..6)?))
}

pub fn rgb_to_hex(color: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.0, color.1, color.2)
}

/// One line saying where a TeX input mistake is and why it is one.
pub fn describe_error(source: &str, error: &ParseError) -> String {
    let start = error.span.start.min(source.len());
    let end = error.span.end.min(source.len());
    let fragment = source.get(start..end).unwrap_or("");
    if start >= source.len() {
        return format!("式の末尾: {error}");
    }
    let column = source[..start].chars().count() + 1;
    if fragment.is_empty() {
        format!("{column}文字目: {error}")
    } else {
        format!("{column}文字目「{fragment}」: {error}")
    }
}
