//! The iced application: state and message handling. Layout lives in `view`.
mod graph;
mod style;
mod view;

use super::history::{EditField, History};
use super::model::{Command, Display, Document, KeyCombo, Library, PngSize, SavedExpr, Settings};
use super::output::{ExportFormat, export_bytes, export_rgba, parse_hex_color};
use super::palette::CATEGORIES;
use super::plot::{
    Compiled, PlotResult, build_plot, compile, eval_constant, fit_view, snap_to_curve,
};
use super::storage::{
    self, FileKind, Session, StorageError, app_dir, export_json, import_json, read_tgf,
    write_atomic, write_tgf,
};
use super::theme::app_theme;
use crate::core::ParseErrorKind;
use crate::render::{LineStyle, Rgb, SceneTheme};
use graph::{GraphCache, Hover};
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key};
use iced::widget::{operation, pane_grid, text_editor};
use iced::{Element, Event, Font, Point, Size, Subscription, Task, Theme, event, window};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const UNDO_LIMIT: usize = 200;
const SCREEN_SAMPLES: usize = 1200;
const SNAP_REACH: f32 = 14.0;
const DEFAULT_PANE_RATIO: f32 = 0.36;

/// Launches the application.
pub fn run() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(App::theme)
        // Not a Japanese font: those draw TeX's backslash as a yen sign.
        // Japanese text falls back to the system's Japanese font.
        .default_font(Font::with_name("Segoe UI"))
        .window_size((1360.0, 860.0))
        .antialiasing(true)
        .exit_on_close_request(false)
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Expressions,
    Graph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Library,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayField {
    Grid,
    Ticks,
    AxisLabels,
    Legend,
    Points,
}

#[derive(Debug, Clone)]
pub enum Message {
    Command(Command),
    Editor(u64, text_editor::Action),
    EditorEnter(u64),
    Remove(u64),
    VisibleToggled(u64, bool),
    ToggleExpanded(u64),
    ColorPicked(u64, Rgb),
    HexEdited(u64, String),
    WidthChanged(u64, f32),
    StyleChanged(u64, LineStyle),
    DerivativeToggled(u64, bool),
    IntegralToggled(u64, bool),
    BoundEdited(u64, bool, String),
    CurveRangeEdited(u64, bool, String),
    PaletteCategory(usize),
    PaletteInsert(usize, usize),
    ParamChanged(String, f64),
    SliderReleased,
    ParamEdited(String, ParamField, String),
    DisplayToggled(DisplayField, bool),
    TransparentToggled(bool),
    PngSizeChanged(PngSize),
    AutoFitToggled(bool),
    RangeEdited(usize, String),
    Zoom { x: f64, y: f64, factor: f64 },
    Pan { dx: f64, dy: f64 },
    Hover(Option<(Point, Size)>),
    PaneResized(pane_grid::ResizeEvent),
    OpenOverlay(Overlay),
    CloseOverlay,
    LibraryQuery(String),
    LibraryFavoritesOnly(bool),
    SaveNameEdited(String),
    SaveTagsEdited(String),
    SaveCurrent,
    SavedAdd(usize),
    SavedDelete(usize),
    SavedFavorite(usize),
    SavedNameEdited(usize, String),
    SavedTagsEdited(usize, String),
    HistoryAdd(usize),
    CaptureShortcut(Command),
    ResetShortcuts,
    JsonExport,
    JsonImport,
    SavePicked(Option<PathBuf>),
    OpenPicked(Option<PathBuf>),
    ExportPicked(Option<PathBuf>),
    JsonExportPicked(Option<PathBuf>),
    JsonImportPicked(Option<PathBuf>),
    KeyPressed(KeyCombo),
    AutosaveTick,
    CloseRequested(window::Id),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamField {
    Value,
    Min,
    Max,
}

pub struct App {
    theme: Theme,
    doc: Document,
    compiled: Compiled,
    plot: PlotResult,
    cache: GraphCache,
    editors: HashMap<u64, text_editor::Content>,
    undo: History<Document>,
    library: Library,
    settings: Settings,
    path: Option<PathBuf>,
    modified: bool,
    session_dirty: bool,
    library_dirty: bool,
    panes: pane_grid::State<PaneKind>,
    pane_ratio: f32,
    active: Option<u64>,
    expanded: HashSet<u64>,
    palette_category: usize,
    /// Text being typed into numeric fields, by field key, until replaced by real values.
    drafts: HashMap<String, String>,
    hover: Option<Hover>,
    overlay: Option<Overlay>,
    library_query: String,
    library_favorites_only: bool,
    save_name: String,
    save_tags: String,
    capturing: Option<Command>,
    status: Option<(String, bool)>,
}

fn panes_with_ratio(ratio: f32) -> pane_grid::State<PaneKind> {
    pane_grid::State::with_configuration(pane_grid::Configuration::Split {
        axis: pane_grid::Axis::Vertical,
        ratio,
        a: Box::new(pane_grid::Configuration::Pane(PaneKind::Expressions)),
        b: Box::new(pane_grid::Configuration::Pane(PaneKind::Graph)),
    })
}

fn screen_plot(doc: &Document, compiled: &Compiled) -> PlotResult {
    build_plot(doc, compiled, SceneTheme::dark(), SCREEN_SAMPLES)
}

fn editor_id(id: u64) -> iced::widget::Id {
    iced::widget::Id::from(format!("expr-{id}"))
}

fn session_path() -> PathBuf {
    app_dir().join("session.tgf")
}

fn library_path() -> PathBuf {
    app_dir().join("library.tgf")
}

fn settings_path() -> PathBuf {
    app_dir().join("settings.tgf")
}

/// Loads the last session. An unreadable one is set aside, never overwritten.
fn load_session() -> (Option<Session>, Option<String>) {
    let path = session_path();
    if !path.exists() {
        return (None, None);
    }
    match read_tgf::<Session>(FileKind::Session, &path) {
        Ok(session) => (Some(session), None),
        Err(e) => {
            let _ = std::fs::rename(&path, path.with_extension("tgf.bad"));
            (None, Some(format!("前回の状態を復元できませんでした: {e}")))
        }
    }
}

fn load_settings() -> Settings {
    std::fs::read(settings_path())
        .ok()
        .and_then(|bytes| storage::decode_settings(&bytes).ok())
        .unwrap_or_default()
}

fn load_or_default<T: serde::de::DeserializeOwned + Default>(kind: FileKind, path: &Path) -> T {
    read_tgf(kind, path).unwrap_or_default()
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let (session, status) = load_session();
        let (doc, path, ratio) = match session {
            Some(s) => (s.document, s.path.map(PathBuf::from), s.pane_ratio),
            None => {
                let mut doc = Document::empty();
                doc.add_entry("\\sin x");
                doc.sync_params();
                (doc, None, DEFAULT_PANE_RATIO)
            }
        };
        let compiled = compile(&doc);
        let plot = screen_plot(&doc, &compiled);
        let editors = doc
            .entries
            .iter()
            .map(|e| (e.id, content_at_end(&e.source)))
            .collect();
        let active = doc.entries.first().map(|e| e.id);
        let mut app = Self {
            theme: app_theme(),
            doc,
            compiled,
            plot,
            cache: GraphCache::new(),
            editors,
            undo: History::new(UNDO_LIMIT),
            library: load_or_default(FileKind::Library, &library_path()),
            settings: load_settings(),
            path,
            modified: false,
            session_dirty: false,
            library_dirty: false,
            panes: panes_with_ratio(ratio),
            pane_ratio: ratio,
            active,
            expanded: HashSet::new(),
            palette_category: 0,
            drafts: HashMap::new(),
            hover: None,
            overlay: None,
            library_query: String::new(),
            library_favorites_only: false,
            save_name: String::new(),
            save_tags: String::new(),
            capturing: None,
            status: status.map(|s| (s, true)),
        };
        app.refresh();
        let focus = app.focus_active();
        (app, focus)
    }

    fn title(&self) -> String {
        let name = self
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(|| "無題".to_string(), |n| n.to_string_lossy().into_owned());
        let mark = if self.modified { " *" } else { "" };
        format!("{name}{mark} - LexGraph")
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            event::listen_with(|event, _status, _window| match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    physical_key,
                    modifiers,
                    ..
                }) => key_combo(&key, physical_key, modifiers).map(Message::KeyPressed),
                _ => None,
            }),
            iced::time::every(Duration::from_secs(2)).map(|_| Message::AutosaveTick),
            window::close_requests().map(Message::CloseRequested),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        view::root(self)
    }

    /// Recomputes everything derived from the document after any edit.
    fn refresh(&mut self) {
        self.compiled = compile(&self.doc);
        self.doc.sync_params();
        if self.doc.auto_fit {
            self.doc.viewport = fit_view(&self.doc, &self.compiled);
            self.clear_range_drafts();
        }
        self.replot();
    }

    fn replot(&mut self) {
        self.plot = screen_plot(&self.doc, &self.compiled);
        self.cache.clear();
        self.session_dirty = true;
    }

    /// These drafts are keyed by list position, which deleting a row shifts.
    fn clear_saved_tags_drafts(&mut self) {
        self.drafts
            .retain(|k, _| !k.starts_with(view::SAVED_TAGS_DRAFT_PREFIX));
    }

    fn clear_range_drafts(&mut self) {
        self.drafts.retain(|k, _| !k.starts_with("range:"));
    }

    fn mark_modified(&mut self) {
        self.modified = true;
        self.session_dirty = true;
    }

    fn set_status(&mut self, text: impl Into<String>) {
        self.status = Some((text.into(), false));
    }

    fn set_error(&mut self, text: impl std::fmt::Display) {
        self.status = Some((text.to_string(), true));
    }

    fn focus_active(&self) -> Task<Message> {
        self.active
            .map_or_else(Task::none, |id| operation::focus(editor_id(id)))
    }

    /// Records an undo step, applies `edit`, and refreshes.
    fn edit_document(&mut self, edit: impl FnOnce(&mut Document)) {
        self.undo.record(self.doc.clone());
        edit(&mut self.doc);
        self.mark_modified();
        self.refresh();
    }

    /// Replaces the document wholesale and rebuilds the input fields to match.
    fn sync_editors(&mut self) {
        self.editors
            .retain(|id, _| self.doc.entries.iter().any(|e| e.id == *id));
        for entry in &self.doc.entries {
            let current = self.editors.get(&entry.id).map(editor_text);
            if current.as_deref() != Some(entry.source.as_str()) {
                self.editors.insert(entry.id, content_at_end(&entry.source));
            }
        }
        if !self
            .active
            .is_some_and(|id| self.doc.entries.iter().any(|e| e.id == id))
        {
            self.active = self.doc.entries.first().map(|e| e.id);
        }
        self.drafts.clear();
    }

    fn remember_valid_source(&mut self, id: u64) {
        let Some(entry) = self.doc.entries.iter().find(|e| e.id == id) else {
            return;
        };
        if self.compiled.formula_of(id).is_some() {
            let source = entry.source.clone();
            self.library.record_history(&source);
            self.library_dirty = true;
        }
    }

    fn switch_active(&mut self, id: u64) {
        if self.active != Some(id) {
            if let Some(previous) = self.active {
                self.remember_valid_source(previous);
            }
            self.active = Some(id);
        }
    }

    fn add_expression(&mut self, source: &str) -> Task<Message> {
        self.undo.record(self.doc.clone());
        if let Some(previous) = self.active {
            self.remember_valid_source(previous);
        }
        let id = self.doc.add_entry(source);
        self.editors.insert(id, content_at_end(source));
        self.active = Some(id);
        self.mark_modified();
        self.refresh();
        self.focus_active()
    }

    fn save_side_files(&mut self) {
        if self.library_dirty {
            match write_tgf(FileKind::Library, &library_path(), &self.library) {
                Ok(()) => self.library_dirty = false,
                Err(e) => self.set_error(e),
            }
        }
    }

    fn save_session(&mut self) {
        if !self.session_dirty {
            return;
        }
        let session = Session {
            document: self.doc.clone(),
            path: self.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            pane_ratio: self.pane_ratio,
        };
        match write_tgf(FileKind::Session, &session_path(), &session) {
            Ok(()) => self.session_dirty = false,
            Err(e) => self.set_error(e),
        }
    }

    fn save_settings(&mut self) {
        if let Err(e) = write_tgf(FileKind::Settings, &settings_path(), &self.settings) {
            self.set_error(e);
        }
    }

    fn save_document_to(&mut self, path: PathBuf) {
        match write_tgf(FileKind::Document, &path, &self.doc) {
            Ok(()) => {
                self.set_status(format!("保存しました: {}", path.display()));
                self.path = Some(path);
                self.modified = false;
                self.session_dirty = true;
            }
            Err(e) => self.set_error(e),
        }
    }

    fn open_document(&mut self, path: PathBuf) {
        match read_tgf::<Document>(FileKind::Document, &path) {
            Ok(document) => {
                self.undo.record(self.doc.clone());
                self.doc = document;
                self.sync_editors();
                self.set_status(format!("開きました: {}", path.display()));
                self.path = Some(path);
                self.modified = false;
                self.session_dirty = true;
                self.refresh();
            }
            Err(e) => self.set_error(e),
        }
    }

    fn export_to(&mut self, path: &Path) {
        let Some(format) = ExportFormat::from_path(path) else {
            self.set_error("拡張子は .png / .svg / .pdf のいずれかにしてください");
            return;
        };
        let transparent = self.settings.transparent_export;
        let size = self.settings.png_size;
        let written = export_bytes(&self.doc, &self.compiled, format, transparent, size)
            .map_err(|e| e.to_string())
            .and_then(|bytes| write_atomic(path, &bytes).map_err(|e| e.to_string()));
        match written {
            Ok(()) => self.set_status(format!("書き出しました: {}", path.display())),
            Err(e) => self.set_error(e),
        }
    }

    fn copy_graph(&mut self) {
        let transparent = self.settings.transparent_export;
        let size = self.settings.png_size;
        let copied = export_rgba(&self.doc, &self.compiled, transparent, size)
            .map_err(|e| e.to_string())
            .and_then(|image| {
                let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
                clipboard
                    .set_image(arboard::ImageData {
                        width: image.width as usize,
                        height: image.height as usize,
                        bytes: image.pixels.into(),
                    })
                    .map_err(|e| e.to_string())
            });
        match copied {
            Ok(()) => self.set_status("グラフをクリップボードにコピーしました"),
            Err(e) => self.set_error(format!("コピーできませんでした: {e}")),
        }
    }

    fn undo_or_redo(&mut self, redo: bool) {
        let current = self.doc.clone();
        let restored = if redo {
            self.undo.redo(current)
        } else {
            self.undo.undo(current)
        };
        let Some(mut restored) = restored else {
            return;
        };
        // The view is navigation, not an edit: undo leaves it where it is.
        restored.viewport = self.doc.viewport;
        restored.auto_fit = self.doc.auto_fit;
        self.doc = restored;
        self.sync_editors();
        self.mark_modified();
        self.refresh();
    }

    fn run_command(&mut self, command: Command) -> Task<Message> {
        match command {
            Command::New => {
                self.undo.record(self.doc.clone());
                let mut doc = Document::empty();
                doc.add_entry("");
                self.doc = doc;
                self.path = None;
                self.modified = false;
                self.sync_editors();
                self.refresh();
                self.set_status("新規ドキュメント（Ctrl+Z で元に戻せます）");
                return self.focus_active();
            }
            Command::Open => return pick_file(tgf_dialog().pick_file(), Message::OpenPicked),
            Command::Save => match self.path.clone() {
                Some(path) => self.save_document_to(path),
                None => return self.run_command(Command::SaveAs),
            },
            Command::SaveAs => {
                let dialog = tgf_dialog().set_file_name("graph.tgf").save_file();
                return pick_file(dialog, Message::SavePicked);
            }
            Command::Undo => self.undo_or_redo(false),
            Command::Redo => self.undo_or_redo(true),
            Command::AddExpression => return self.add_expression(""),
            Command::Library => {
                self.clear_saved_tags_drafts();
                self.overlay = Some(Overlay::Library);
            }
            Command::Export => {
                let dialog = rfd::AsyncFileDialog::new()
                    .add_filter("PNG 画像", &["png"])
                    .add_filter("SVG 画像", &["svg"])
                    .add_filter("PDF", &["pdf"])
                    .set_file_name("graph.png")
                    .save_file();
                return pick_file(dialog, Message::ExportPicked);
            }
            Command::CopyGraph => self.copy_graph(),
            Command::FitView => {
                self.doc.viewport = fit_view(&self.doc, &self.compiled);
                self.clear_range_drafts();
                self.replot();
            }
            Command::ToggleLegend => {
                self.doc.display.show_legend = !self.doc.display.show_legend;
                self.mark_modified();
                self.replot();
            }
        }
        Task::none()
    }

    fn handle_key(&mut self, combo: KeyCombo) -> Task<Message> {
        if let Some(command) = self.capturing {
            if combo.key == "escape" {
                self.capturing = None;
                return Task::none();
            }
            let plain_text_key = !combo.ctrl && !combo.alt && combo.key.chars().count() == 1;
            if plain_text_key {
                self.set_error("文字キーには Ctrl か Alt を組み合わせてください");
                return Task::none();
            }
            match self.settings.shortcuts.rebind(command, combo) {
                Ok(()) => {
                    self.capturing = None;
                    self.save_settings();
                    self.set_status("ショートカットを変更しました");
                }
                Err(other) => {
                    self.set_error(format!("そのキーは「{}」で使用中です", other.title()));
                }
            }
            return Task::none();
        }
        if combo.key == "escape" && self.overlay.is_some() {
            self.overlay = None;
            return Task::none();
        }
        match self.settings.shortcuts.lookup(&combo) {
            Some(command) => self.run_command(command),
            None => Task::none(),
        }
    }

    fn edit_expression(&mut self, id: u64, action: text_editor::Action) {
        self.switch_active(id);
        let is_edit = action.is_edit();
        let Some(content) = self.editors.get_mut(&id) else {
            return;
        };
        content.perform(action);
        if !is_edit {
            return;
        }
        let mut source = editor_text(content);
        if source.contains('\n') {
            // A pasted line break cannot be part of a formula.
            source = source.replace(['\r', '\n'], " ");
            *content = content_at_end(&source);
        }
        self.undo
            .record_coalesced(EditField::Source(id).key(), self.doc.clone());
        if let Some(entry) = self.doc.entry_mut(id) {
            entry.source = source;
        }
        self.mark_modified();
        self.refresh();
    }

    fn insert_palette(&mut self, category: usize, index: usize) -> Task<Message> {
        let Some(id) = self
            .active
            .or_else(|| self.doc.entries.first().map(|e| e.id))
        else {
            return Task::none();
        };
        let Some(item) = CATEGORIES.get(category).and_then(|c| c.items.get(index)) else {
            return Task::none();
        };
        self.active = Some(id);
        let paste =
            text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new(item.insert.to_string())));
        self.undo.record(self.doc.clone());
        if let Some(content) = self.editors.get_mut(&id) {
            content.perform(paste);
            for _ in 0..item.cursor_back {
                content.perform(text_editor::Action::Move(text_editor::Motion::Left));
            }
            let source = editor_text(content);
            if let Some(entry) = self.doc.entry_mut(id) {
                entry.source = source;
            }
        }
        self.mark_modified();
        self.refresh();
        operation::focus(editor_id(id))
    }

    fn set_display(&mut self, field: DisplayField, on: bool) {
        let Display {
            show_grid,
            show_ticks,
            show_axis_labels,
            show_legend,
            show_points,
        } = &mut self.doc.display;
        *match field {
            DisplayField::Grid => show_grid,
            DisplayField::Ticks => show_ticks,
            DisplayField::AxisLabels => show_axis_labels,
            DisplayField::Legend => show_legend,
            DisplayField::Points => show_points,
        } = on;
        self.mark_modified();
        self.replot();
    }

    fn edit_range(&mut self, index: usize, text: String) {
        let parsed = eval_constant(&text);
        self.drafts.insert(format!("range:{index}"), text);
        let Some(value) = parsed else {
            return;
        };
        let mut vp = self.doc.viewport;
        match index {
            0 => vp.x_min = value,
            1 => vp.x_max = value,
            2 => vp.y_min = value,
            _ => vp.y_max = value,
        }
        if vp.is_valid() {
            self.doc.viewport = vp;
            self.doc.auto_fit = false;
            self.mark_modified();
            self.replot();
        }
    }

    fn edit_param(&mut self, name: String, field: ParamField, text: String) {
        let parsed = eval_constant(&text);
        self.drafts
            .insert(view::param_draft_key(&name, field), text);
        let Some(value) = parsed else {
            return;
        };
        self.undo
            .record_coalesced(EditField::Param(&name, field as u8).key(), self.doc.clone());
        if let Some(setting) = self.doc.params.get_mut(&name) {
            match field {
                ParamField::Value => setting.value = value,
                ParamField::Min => setting.min = value,
                ParamField::Max => setting.max = value,
            }
        }
        self.mark_modified();
        self.refresh();
    }

    fn apply_import(&mut self, path: &Path) {
        let bundle = std::fs::read_to_string(path)
            .map_err(StorageError::from)
            .and_then(|text| import_json(&text));
        match bundle {
            Ok(bundle) => {
                let added = self.library.merge(bundle.saved);
                self.settings = bundle.settings;
                self.library_dirty = true;
                self.save_settings();
                self.save_side_files();
                self.set_status(format!(
                    "読み込みました（新しい保存式 {added} 件、設定を更新）"
                ));
            }
            Err(e) => self.set_error(e),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Command(command) => return self.run_command(command),
            Message::Editor(id, action) => self.edit_expression(id, action),
            Message::EditorEnter(_) => return self.add_expression(""),
            Message::Remove(id) => {
                self.edit_document(|doc| doc.remove_entry(id));
                self.sync_editors();
            }
            Message::VisibleToggled(id, on) => self.edit_entry(id, |e| e.visible = on),
            Message::ToggleExpanded(id) => {
                if !self.expanded.remove(&id) {
                    self.expanded.insert(id);
                }
            }
            Message::ColorPicked(id, color) => {
                self.drafts.remove(&view::hex_draft_key(id));
                self.edit_entry(id, |e| e.color = color);
            }
            Message::HexEdited(id, text) => {
                let color = parse_hex_color(&text);
                self.drafts.insert(view::hex_draft_key(id), text);
                if let Some(color) = color {
                    self.edit_entry(id, |e| e.color = color);
                }
            }
            Message::WidthChanged(id, width) => {
                self.undo
                    .record_coalesced(EditField::Width(id).key(), self.doc.clone());
                if let Some(entry) = self.doc.entry_mut(id) {
                    entry.width = width;
                }
                self.mark_modified();
                self.replot();
            }
            Message::StyleChanged(id, style) => self.edit_entry(id, |e| e.style = style),
            Message::DerivativeToggled(id, on) => self.edit_entry(id, |e| e.show_derivative = on),
            Message::IntegralToggled(id, on) => self.edit_entry(id, |e| e.integral.enabled = on),
            Message::BoundEdited(id, is_a, text) => {
                self.undo
                    .record_coalesced(EditField::IntegralBound(id, is_a).key(), self.doc.clone());
                if let Some(entry) = self.doc.entry_mut(id) {
                    if is_a {
                        entry.integral.a = text;
                    } else {
                        entry.integral.b = text;
                    }
                }
                self.mark_modified();
                self.refresh();
            }
            Message::CurveRangeEdited(id, is_min, text) => {
                let parsed = eval_constant(&text);
                self.drafts.insert(view::curve_draft_key(id, is_min), text);
                if let Some(value) = parsed {
                    self.undo.record_coalesced(
                        EditField::CurveRange(id, is_min).key(),
                        self.doc.clone(),
                    );
                    if let Some(entry) = self.doc.entry_mut(id) {
                        if is_min {
                            entry.t_min = value;
                        } else {
                            entry.t_max = value;
                        }
                    }
                    self.mark_modified();
                    self.refresh();
                }
            }
            Message::PaletteCategory(index) => self.palette_category = index,
            Message::PaletteInsert(category, index) => {
                return self.insert_palette(category, index);
            }
            Message::ParamChanged(name, value) => {
                self.undo.record_coalesced(
                    EditField::Param(&name, ParamField::Value as u8).key(),
                    self.doc.clone(),
                );
                self.drafts
                    .remove(&view::param_draft_key(&name, ParamField::Value));
                if let Some(setting) = self.doc.params.get_mut(&name) {
                    setting.value = value;
                }
                self.mark_modified();
                self.refresh();
            }
            Message::SliderReleased => self.undo.end_coalescing(),
            Message::ParamEdited(name, field, text) => self.edit_param(name, field, text),
            Message::DisplayToggled(field, on) => self.set_display(field, on),
            Message::PngSizeChanged(size) => {
                self.settings.png_size = size;
                self.save_settings();
            }
            Message::TransparentToggled(on) => {
                self.settings.transparent_export = on;
                self.save_settings();
            }
            Message::AutoFitToggled(on) => {
                self.doc.auto_fit = on;
                self.mark_modified();
                self.refresh();
            }
            Message::RangeEdited(index, text) => self.edit_range(index, text),
            Message::Zoom { x, y, factor } => {
                self.doc.viewport = self.doc.viewport.zoom_at(x, y, factor);
                self.after_navigation();
            }
            Message::Pan { dx, dy } => {
                self.doc.viewport = self.doc.viewport.panned(dx, dy);
                self.after_navigation();
            }
            Message::Hover(position) => {
                self.hover = position.map(|(at, size)| {
                    let snapped = snap_to_curve(
                        &self.plot.spec,
                        (at.x, at.y),
                        size.width,
                        size.height,
                        SNAP_REACH,
                    );
                    match snapped {
                        Some(s) => Hover {
                            x: s.x,
                            y: s.y,
                            curve: Some(s.curve),
                        },
                        None => {
                            let (x, y) =
                                self.doc
                                    .viewport
                                    .from_px(at.x, at.y, size.width, size.height);
                            Hover { x, y, curve: None }
                        }
                    }
                });
            }
            Message::PaneResized(event) => {
                self.panes.resize(event.split, event.ratio);
                self.pane_ratio = event.ratio;
                self.session_dirty = true;
            }
            Message::OpenOverlay(overlay) => {
                self.clear_saved_tags_drafts();
                self.overlay = Some(overlay);
            }
            Message::CloseOverlay => {
                self.overlay = None;
                self.capturing = None;
            }
            Message::LibraryQuery(text) => self.library_query = text,
            Message::LibraryFavoritesOnly(on) => self.library_favorites_only = on,
            Message::SaveNameEdited(text) => self.save_name = text,
            Message::SaveTagsEdited(text) => self.save_tags = text,
            Message::SaveCurrent => self.save_current_expression(),
            Message::SavedAdd(index) => {
                if let Some(source) = self.library.saved.get(index).map(|s| s.source.clone()) {
                    self.overlay = None;
                    return self.add_expression(&source);
                }
            }
            Message::SavedDelete(index) => {
                if index < self.library.saved.len() {
                    self.library.saved.remove(index);
                    self.library_dirty = true;
                    self.clear_saved_tags_drafts();
                }
            }
            Message::SavedFavorite(index) => {
                if let Some(saved) = self.library.saved.get_mut(index) {
                    saved.favorite = !saved.favorite;
                    self.library_dirty = true;
                }
            }
            Message::SavedNameEdited(index, text) => {
                if let Some(saved) = self.library.saved.get_mut(index) {
                    saved.name = text;
                    self.library_dirty = true;
                }
            }
            Message::SavedTagsEdited(index, text) => {
                if let Some(saved) = self.library.saved.get_mut(index) {
                    saved.tags = split_tags(&text);
                    self.library_dirty = true;
                    // The field keeps the raw text: a comma just typed splits no tag yet.
                    self.drafts.insert(view::saved_tags_draft_key(index), text);
                }
            }
            Message::HistoryAdd(index) => {
                if let Some(source) = self.library.history.get(index).cloned() {
                    self.overlay = None;
                    return self.add_expression(&source);
                }
            }
            Message::CaptureShortcut(command) => self.capturing = Some(command),
            Message::ResetShortcuts => {
                self.settings.shortcuts = Default::default();
                self.capturing = None;
                self.save_settings();
                self.set_status("ショートカットを初期状態に戻しました");
            }
            Message::JsonExport => {
                let dialog = json_dialog()
                    .set_file_name("lexgraph-export.json")
                    .save_file();
                return pick_file(dialog, Message::JsonExportPicked);
            }
            Message::JsonImport => {
                return pick_file(json_dialog().pick_file(), Message::JsonImportPicked);
            }
            Message::SavePicked(Some(path)) => self.save_document_to(path),
            Message::OpenPicked(Some(path)) => self.open_document(path),
            Message::ExportPicked(Some(path)) => self.export_to(&path),
            Message::JsonExportPicked(Some(path)) => {
                let written = export_json(&self.library, &self.settings)
                    .and_then(|text| storage::write_atomic(&path, text.as_bytes()));
                match written {
                    Ok(()) => self.set_status(format!("書き出しました: {}", path.display())),
                    Err(e) => self.set_error(e),
                }
            }
            Message::JsonImportPicked(Some(path)) => self.apply_import(&path),
            Message::SavePicked(None)
            | Message::OpenPicked(None)
            | Message::ExportPicked(None)
            | Message::JsonExportPicked(None)
            | Message::JsonImportPicked(None) => {}
            Message::KeyPressed(combo) => return self.handle_key(combo),
            Message::AutosaveTick => {
                self.save_session();
                self.save_side_files();
            }
            Message::CloseRequested(id) => {
                if let Some(active) = self.active {
                    self.remember_valid_source(active);
                }
                self.session_dirty = true;
                self.save_session();
                self.save_side_files();
                return window::close(id);
            }
        }
        Task::none()
    }

    fn edit_entry(&mut self, id: u64, edit: impl FnOnce(&mut super::model::ExprEntry)) {
        self.edit_document(|doc| {
            if let Some(entry) = doc.entry_mut(id) {
                edit(entry);
            }
        });
    }

    fn after_navigation(&mut self) {
        self.doc.auto_fit = false;
        self.hover = None;
        self.clear_range_drafts();
        self.mark_modified();
        self.replot();
    }

    fn save_current_expression(&mut self) {
        let Some(entry) = self
            .active
            .and_then(|id| self.doc.entries.iter().find(|e| e.id == id))
        else {
            self.set_error("保存する式を選んでください");
            return;
        };
        if entry.source.trim().is_empty() {
            self.set_error("空の式は保存できません");
            return;
        }
        let name = if self.save_name.trim().is_empty() {
            entry.source.trim().to_string()
        } else {
            self.save_name.trim().to_string()
        };
        self.library.saved.push(SavedExpr {
            name,
            source: entry.source.clone(),
            tags: split_tags(&self.save_tags),
            favorite: false,
        });
        self.save_name.clear();
        self.save_tags.clear();
        self.library_dirty = true;
        self.set_status("式を保存しました");
    }

    /// True if an empty entry's "no input" error should stay hidden.
    fn visible_error(&self, id: u64) -> Option<&crate::core::ParseError> {
        self.compiled
            .error_of(id)
            .filter(|e| e.kind != ParseErrorKind::Empty)
    }
}

/// A text field holding `text`, with the cursor after it, where typing continues.
fn content_at_end(text: &str) -> text_editor::Content {
    let mut content = text_editor::Content::with_text(text);
    content.perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
    content
}

fn editor_text(content: &text_editor::Content) -> String {
    content.text().trim_end_matches(['\r', '\n']).to_string()
}

fn split_tags(text: &str) -> Vec<String> {
    text.split([',', '、'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

fn tgf_dialog() -> rfd::AsyncFileDialog {
    rfd::AsyncFileDialog::new().add_filter("LexGraph", &["tgf"])
}

fn json_dialog() -> rfd::AsyncFileDialog {
    rfd::AsyncFileDialog::new().add_filter("JSON", &["json"])
}

fn pick_file<F>(dialog: F, to_message: fn(Option<PathBuf>) -> Message) -> Task<Message>
where
    F: std::future::Future<Output = Option<rfd::FileHandle>> + Send + 'static,
{
    Task::perform(
        async move { dialog.await.map(|handle| handle.path().to_path_buf()) },
        to_message,
    )
}

/// Names a key press for shortcut matching. Letter keys are named by their Latin
/// letter so shortcuts keep working while a Japanese IME is on.
fn key_combo(
    key: &Key,
    physical: keyboard::key::Physical,
    modifiers: keyboard::Modifiers,
) -> Option<KeyCombo> {
    let name = match key.as_ref() {
        Key::Named(named) => named_key(named)?.to_string(),
        _ => key.to_latin(physical)?.to_lowercase().to_string(),
    };
    Some(KeyCombo {
        ctrl: modifiers.control(),
        shift: modifiers.shift(),
        alt: modifiers.alt(),
        key: name,
    })
}

fn named_key(named: Named) -> Option<&'static str> {
    Some(match named {
        Named::Enter => "enter",
        Named::Tab => "tab",
        Named::Escape => "escape",
        Named::Delete => "delete",
        Named::Insert => "insert",
        Named::Home => "home",
        Named::End => "end",
        Named::PageUp => "pageup",
        Named::PageDown => "pagedown",
        Named::F1 => "f1",
        Named::F2 => "f2",
        Named::F3 => "f3",
        Named::F4 => "f4",
        Named::F5 => "f5",
        Named::F6 => "f6",
        Named::F7 => "f7",
        Named::F8 => "f8",
        Named::F9 => "f9",
        Named::F10 => "f10",
        Named::F11 => "f11",
        Named::F12 => "f12",
        _ => return None,
    })
}
