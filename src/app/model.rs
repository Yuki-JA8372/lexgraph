//! The data the user edits and keeps. Everything here is serializable.
use crate::core::{FormulaKind, parse, parse_expression};
use crate::render::{LineStyle, Rgb, Viewport};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

/// Automatic curve colors, chosen to read on both the dark screen and white exports.
pub const PALETTE: [Rgb; 8] = [
    Rgb(214, 64, 69),
    Rgb(40, 120, 220),
    Rgb(30, 140, 90),
    Rgb(200, 110, 20),
    Rgb(140, 90, 220),
    Rgb(0, 140, 150),
    Rgb(210, 70, 150),
    Rgb(120, 130, 30),
];

const HISTORY_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct IntegralRange {
    pub enabled: bool,
    pub a: String,
    pub b: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExprEntry {
    pub id: u64,
    pub source: String,
    pub color: Rgb,
    pub width: f32,
    pub style: LineStyle,
    pub visible: bool,
    pub show_derivative: bool,
    pub integral: IntegralRange,
    /// Parameter range for parametric and polar curves.
    pub t_min: f64,
    pub t_max: f64,
}

impl ExprEntry {
    /// Names used by this entry's integral bounds while its fill is on, sorted.
    pub fn bound_params(&self) -> Vec<String> {
        if !self.integral.enabled {
            return Vec::new();
        }
        let mut names = BTreeSet::new();
        for bound in [&self.integral.a, &self.integral.b] {
            if let Ok(expr) = parse_expression(bound) {
                expr.collect_vars(&mut names);
            }
        }
        names.into_iter().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParamSetting {
    pub value: f64,
    pub min: f64,
    pub max: f64,
}

impl Default for ParamSetting {
    fn default() -> Self {
        Self {
            value: 1.0,
            min: -10.0,
            max: 10.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Display {
    pub show_grid: bool,
    pub show_ticks: bool,
    pub show_axis_labels: bool,
    pub show_legend: bool,
    /// Automatic intersection, extremum and root markers.
    pub show_points: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            show_grid: true,
            show_ticks: true,
            show_axis_labels: true,
            show_legend: true,
            show_points: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub entries: Vec<ExprEntry>,
    pub params: BTreeMap<String, ParamSetting>,
    pub viewport: Viewport,
    pub display: Display,
    /// Recompute the visible range automatically; manual pan, zoom or range entry turns it off.
    pub auto_fit: bool,
    pub next_id: u64,
}

impl Document {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            params: BTreeMap::new(),
            viewport: Viewport::default(),
            display: Display::default(),
            auto_fit: true,
            next_id: 1,
        }
    }

    fn next_color(&self) -> Rgb {
        PALETTE
            .iter()
            .copied()
            .find(|c| self.entries.iter().all(|e| e.color != *c))
            .unwrap_or(PALETTE[self.entries.len() % PALETTE.len()])
    }

    /// Adds an expression with automatic styling and returns its id.
    pub fn add_entry(&mut self, source: &str) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let color = self.next_color();
        self.entries.push(ExprEntry {
            id,
            source: source.to_string(),
            color,
            width: 2.2,
            style: LineStyle::Solid,
            visible: true,
            show_derivative: false,
            integral: IntegralRange {
                enabled: false,
                a: "0".into(),
                b: "1".into(),
            },
            t_min: 0.0,
            t_max: TAU,
        });
        id
    }

    pub fn remove_entry(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
    }

    pub fn entry_mut(&mut self, id: u64) -> Option<&mut ExprEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Names to offer sliders for: the free names of every valid formula and
    /// of the integral bounds in use, sorted.
    pub fn detected_params(&self) -> Vec<String> {
        let mut names = BTreeSet::new();
        for entry in &self.entries {
            let Ok(formula) = parse(&entry.source) else {
                continue;
            };
            names.extend(formula.params.iter().cloned());
            if matches!(formula.kind, FormulaKind::Cartesian(_)) {
                names.extend(entry.bound_params());
            }
        }
        names.into_iter().collect()
    }

    /// Adds settings for newly detected parameters. Settings of names no longer
    /// in use stay, so retyping a name brings its value back.
    pub fn sync_params(&mut self) {
        for name in self.detected_params() {
            self.params.entry(name).or_default();
        }
    }

    /// Current parameter values for evaluation.
    pub fn param_values(&self) -> BTreeMap<String, f64> {
        self.params
            .iter()
            .map(|(k, v)| (k.clone(), v.value))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedExpr {
    pub name: String,
    pub source: String,
    pub tags: Vec<String>,
    pub favorite: bool,
}

impl SavedExpr {
    fn matches(&self, term: &str) -> bool {
        self.name.to_lowercase().contains(term)
            || self.source.to_lowercase().contains(term)
            || self.tags.iter().any(|t| t.to_lowercase().contains(term))
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Library {
    pub saved: Vec<SavedExpr>,
    pub history: Vec<String>,
}

impl Library {
    /// Indices of the saved expressions matching every whitespace-separated term.
    pub fn search(&self, query: &str, only_favorites: bool) -> Vec<usize> {
        let query = query.to_lowercase();
        let terms: Vec<&str> = query.split_whitespace().collect();
        self.saved
            .iter()
            .enumerate()
            .filter(|(_, s)| !only_favorites || s.favorite)
            .filter(|(_, s)| terms.iter().all(|t| s.matches(t)))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn record_history(&mut self, source: &str) {
        let source = source.trim();
        if source.is_empty() {
            return;
        }
        self.history.retain(|h| h != source);
        self.history.insert(0, source.to_string());
        self.history.truncate(HISTORY_LIMIT);
    }

    /// Adds expressions not already present; returns how many were added.
    pub fn merge(&mut self, incoming: Vec<SavedExpr>) -> usize {
        let mut added = 0;
        for item in incoming {
            let exists = self
                .saved
                .iter()
                .any(|s| s.name == item.name && s.source == item.source);
            if !exists {
                self.saved.push(item);
                added += 1;
            }
        }
        added
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    New,
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    AddExpression,
    Library,
    Export,
    CopyGraph,
    FitView,
    ToggleLegend,
}

impl Command {
    pub fn title(self) -> &'static str {
        match self {
            Command::New => "新規",
            Command::Open => "開く",
            Command::Save => "保存",
            Command::SaveAs => "名前を付けて保存",
            Command::Undo => "取り消し",
            Command::Redo => "やり直し",
            Command::AddExpression => "式を追加",
            Command::Library => "式ライブラリ",
            Command::Export => "画像を書き出し",
            Command::CopyGraph => "グラフをコピー",
            Command::FitView => "表示範囲を自動調整",
            Command::ToggleLegend => "凡例の表示切替",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyCombo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Lowercase key name, such as `s`, `0` or `enter`.
    pub key: String,
}

impl KeyCombo {
    pub fn ctrl(key: &str) -> Self {
        Self {
            ctrl: true,
            shift: false,
            alt: false,
            key: key.to_string(),
        }
    }

    pub fn ctrl_shift(key: &str) -> Self {
        Self {
            shift: true,
            ..Self::ctrl(key)
        }
    }

    pub fn label(&self) -> String {
        let mut out = String::new();
        if self.ctrl {
            out.push_str("Ctrl+");
        }
        if self.shift {
            out.push_str("Shift+");
        }
        if self.alt {
            out.push_str("Alt+");
        }
        let mut chars = self.key.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shortcuts {
    pub bindings: Vec<(Command, KeyCombo)>,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            bindings: vec![
                (Command::New, KeyCombo::ctrl("n")),
                (Command::Open, KeyCombo::ctrl("o")),
                (Command::Save, KeyCombo::ctrl("s")),
                (Command::SaveAs, KeyCombo::ctrl_shift("s")),
                (Command::Undo, KeyCombo::ctrl("z")),
                (Command::Redo, KeyCombo::ctrl("y")),
                (Command::AddExpression, KeyCombo::ctrl("enter")),
                (Command::Library, KeyCombo::ctrl("b")),
                (Command::Export, KeyCombo::ctrl("e")),
                (Command::CopyGraph, KeyCombo::ctrl_shift("g")),
                (Command::FitView, KeyCombo::ctrl("0")),
                (Command::ToggleLegend, KeyCombo::ctrl("l")),
            ],
        }
    }
}

impl Shortcuts {
    pub fn lookup(&self, combo: &KeyCombo) -> Option<Command> {
        self.bindings
            .iter()
            .find(|(_, c)| c == combo)
            .map(|(command, _)| *command)
    }

    /// Assigns `combo` to `command`. If another command uses it, returns that
    /// command and changes nothing.
    pub fn rebind(&mut self, command: Command, combo: KeyCombo) -> Result<(), Command> {
        if let Some(other) = self.lookup(&combo).filter(|c| *c != command) {
            return Err(other);
        }
        if let Some(binding) = self.bindings.iter_mut().find(|(c, _)| *c == command) {
            binding.1 = combo;
        } else {
            self.bindings.push((command, combo));
        }
        Ok(())
    }
}

/// Pixel size of PNG files and clipboard images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PngSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl PngSize {
    pub const ALL: [PngSize; 3] = [PngSize::Small, PngSize::Medium, PngSize::Large];

    pub fn pixels(self) -> (u32, u32) {
        match self {
            PngSize::Small => (800, 500),
            PngSize::Medium => (1600, 1000),
            PngSize::Large => (3200, 2000),
        }
    }
}

impl std::fmt::Display for PngSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (width, height) = self.pixels();
        write!(f, "{width}×{height}")
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Settings {
    pub shortcuts: Shortcuts,
    pub transparent_export: bool,
    /// Absent from JSON exported before this setting existed.
    #[serde(default)]
    pub png_size: PngSize,
}
