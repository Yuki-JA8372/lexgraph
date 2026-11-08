//! Turns evaluated formulas into curves, then into a backend-neutral scene that
//! is drawn on screen (`canvas`) or exported (`export`).
pub mod canvas;
pub mod export;
pub mod sampling;
pub mod scene;
pub mod tokens;
pub mod viewport;

pub use export::{
    RenderError, RgbaImage, to_pdf, to_png, to_png_sized, to_rgba, to_rgba_sized, to_svg,
};
pub use sampling::{auto_fit_y, sample_formula};
pub use scene::{
    Anchor, CurveSpec, LineStyle, Marker, MarkerKind, PlotSpec, Prim, Rgb, Scene, SceneTheme,
    build_scene, format_tick, nice_ticks,
};
pub use viewport::Viewport;
