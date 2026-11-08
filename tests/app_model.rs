use lexgraph::app::history::{EditField, History};
use lexgraph::app::model::{
    Command, Document, ExprEntry, KeyCombo, Library, PngSize, SavedExpr, Settings, Shortcuts,
};
use lexgraph::app::plot::{build_plot, compile, snap_to_curve};
use lexgraph::app::storage::{
    self, ExportBundle, FileKind, StorageError, decode, decode_settings, encode, export_json,
    import_json,
};
use lexgraph::render::{LineStyle, MarkerKind, SceneTheme, Viewport};

fn doc_with(sources: &[&str]) -> Document {
    let mut doc = Document::empty();
    for s in sources {
        doc.add_entry(s);
    }
    doc.sync_params();
    doc
}

fn combo(ctrl: bool, shift: bool, key: &str) -> KeyCombo {
    KeyCombo {
        ctrl,
        shift,
        alt: false,
        key: key.into(),
    }
}

// ---- document ----

#[test]
fn new_entries_get_distinct_automatic_colors() {
    let doc = doc_with(&["x", "x+1", "x+2"]);
    let colors: Vec<_> = doc.entries.iter().map(|e| e.color).collect();
    assert_ne!(colors[0], colors[1]);
    assert_ne!(colors[1], colors[2]);
    assert_ne!(colors[0], colors[2]);
}

#[test]
fn color_of_removed_entry_is_reused_before_repeating() {
    let mut doc = doc_with(&["x", "x+1"]);
    let freed = doc.entries[0].color;
    let id = doc.entries[0].id;
    doc.remove_entry(id);
    doc.add_entry("x+2");
    assert_eq!(doc.entries.last().unwrap().color, freed);
}

#[test]
fn entry_ids_are_never_reused() {
    let mut doc = doc_with(&["x"]);
    let first = doc.entries[0].id;
    doc.remove_entry(first);
    assert_ne!(doc.add_entry("x"), first);
}

#[test]
fn params_are_detected_across_entries_in_order() {
    let doc = doc_with(&["b x", "a x + b", "\\frac{"]);
    assert_eq!(
        doc.detected_params(),
        vec!["a".to_string(), "b".to_string()]
    );
    assert!(doc.params.contains_key("a"));
}

#[test]
fn sync_params_keeps_existing_values() {
    let mut doc = doc_with(&["a x"]);
    doc.params.get_mut("a").unwrap().value = 4.0;
    doc.add_entry("a + b");
    doc.sync_params();
    assert_eq!(doc.params["a"].value, 4.0);
    assert_eq!(doc.params["b"].value, 1.0);
}

#[test]
fn bound_params_only_lists_names_in_that_entrys_bounds() {
    let mut doc = doc_with(&["\\sin x", "a x"]);
    doc.entries[0].integral.enabled = true;
    doc.entries[0].integral.a = "0".into();
    doc.entries[0].integral.b = "c\\pi".into();
    assert_eq!(doc.entries[0].bound_params(), vec!["c".to_string()]);
    assert!(doc.entries[1].bound_params().is_empty());
}

#[test]
fn bound_params_is_empty_when_the_fill_is_off() {
    let mut doc = doc_with(&["\\sin x"]);
    doc.entries[0].integral.b = "c".into();
    assert!(doc.entries[0].bound_params().is_empty());
}

// ---- undo ----

#[test]
fn undo_and_redo_walk_states() {
    let mut h = History::new(10);
    h.record(1);
    assert_eq!(h.undo(2), Some(1));
    assert_eq!(h.redo(1), Some(2));
    assert_eq!(h.redo(2), None);
}

#[test]
fn new_edit_clears_redo() {
    let mut h = History::new(10);
    h.record(1);
    assert_eq!(h.undo(2), Some(1));
    h.record(1);
    assert_eq!(h.redo(3), None);
}

#[test]
fn typing_in_one_field_coalesces_into_one_undo_step() {
    let mut h = History::new(10);
    h.record_coalesced(7, "");
    h.record_coalesced(7, "a");
    h.record_coalesced(7, "ab");
    assert_eq!(h.undo("abc"), Some(""));
    assert_eq!(h.undo(""), None);
}

#[test]
fn typing_in_another_field_starts_a_new_step() {
    let mut h = History::new(10);
    h.record_coalesced(1, "s0");
    h.record_coalesced(2, "s1");
    assert_eq!(h.undo("s2"), Some("s1"));
    assert_eq!(h.undo("s1"), Some("s0"));
}

#[test]
fn undo_after_undo_breaks_coalescing() {
    let mut h = History::new(10);
    h.record_coalesced(1, "s0");
    assert_eq!(h.undo("s1"), Some("s0"));
    h.record_coalesced(1, "s0");
    assert_eq!(h.undo("s2"), Some("s0"));
}

#[test]
fn history_drops_oldest_beyond_limit() {
    let mut h = History::new(2);
    h.record(1);
    h.record(2);
    h.record(3);
    assert_eq!(h.undo(4), Some(3));
    assert_eq!(h.undo(3), Some(2));
    assert_eq!(h.undo(2), None);
}

// ---- library ----

fn library() -> Library {
    let mut lib = Library::default();
    lib.saved.push(SavedExpr {
        name: "Parabola".into(),
        source: "x^2".into(),
        tags: vec!["basic".into()],
        favorite: true,
    });
    lib.saved.push(SavedExpr {
        name: "Wave".into(),
        source: "\\sin x".into(),
        tags: vec!["trig".into(), "basic".into()],
        favorite: false,
    });
    lib
}

#[test]
fn search_matches_name_source_and_tags_case_insensitively() {
    let lib = library();
    assert_eq!(lib.search("parab", false), vec![0]);
    assert_eq!(lib.search("\\SIN", false), vec![1]);
    assert_eq!(lib.search("trig", false), vec![1]);
    assert_eq!(lib.search("basic", false), vec![0, 1]);
    assert_eq!(lib.search("", false), vec![0, 1]);
}

#[test]
fn search_terms_are_anded_and_favorites_filter() {
    let lib = library();
    assert_eq!(lib.search("basic wave", false), vec![1]);
    assert_eq!(lib.search("basic", true), vec![0]);
}

#[test]
fn history_is_most_recent_first_without_duplicates_or_blanks() {
    let mut lib = Library::default();
    lib.record_history("x");
    lib.record_history("y=x");
    lib.record_history("x");
    lib.record_history("   ");
    assert_eq!(lib.history, vec!["x".to_string(), "y=x".to_string()]);
}

#[test]
fn history_is_capped() {
    let mut lib = Library::default();
    for i in 0..300 {
        lib.record_history(&format!("x+{i}"));
    }
    assert_eq!(lib.history.len(), 100);
    assert_eq!(lib.history[0], "x+299");
}

// ---- shortcuts ----

#[test]
fn default_shortcuts_cover_the_standard_set() {
    let s = Shortcuts::default();
    assert_eq!(s.lookup(&combo(true, false, "s")), Some(Command::Save));
    assert_eq!(s.lookup(&combo(true, false, "z")), Some(Command::Undo));
    assert_eq!(s.lookup(&combo(true, false, "y")), Some(Command::Redo));
    assert_eq!(s.lookup(&combo(true, false, "o")), Some(Command::Open));
    assert_eq!(s.lookup(&combo(true, true, "s")), Some(Command::SaveAs));
    assert_eq!(s.lookup(&combo(false, false, "s")), None);
}

#[test]
fn rebinding_replaces_the_old_combo() {
    let mut s = Shortcuts::default();
    s.rebind(Command::Save, combo(true, true, "k")).unwrap();
    assert_eq!(s.lookup(&combo(true, true, "k")), Some(Command::Save));
    assert_eq!(s.lookup(&combo(true, false, "s")), None);
}

#[test]
fn rebinding_to_a_used_combo_reports_the_conflict_and_changes_nothing() {
    let mut s = Shortcuts::default();
    let err = s
        .rebind(Command::Save, combo(true, false, "z"))
        .unwrap_err();
    assert_eq!(err, Command::Undo);
    assert_eq!(s.lookup(&combo(true, false, "s")), Some(Command::Save));
}

#[test]
fn rebinding_to_its_own_combo_is_allowed() {
    let mut s = Shortcuts::default();
    assert!(s.rebind(Command::Save, combo(true, false, "s")).is_ok());
}

#[test]
fn combo_label_is_readable() {
    assert_eq!(combo(true, true, "s").label(), "Ctrl+Shift+S");
}

// ---- storage ----

#[test]
fn document_round_trips_through_tgf_with_its_view_and_styles() {
    let mut doc = doc_with(&["a x"]);
    doc.viewport = Viewport {
        x_min: -1.5,
        x_max: 2.5,
        y_min: -3.0,
        y_max: 4.0,
    };
    doc.entries[0].style = LineStyle::Dotted;
    doc.entries[0].width = 3.5;
    doc.params.get_mut("a").unwrap().value = 2.25;
    let bytes = encode(FileKind::Document, &doc).unwrap();
    let back: Document = decode(FileKind::Document, &bytes).unwrap();
    assert_eq!(back, doc);
}

#[test]
fn tgf_header_has_magic_and_version() {
    let bytes = encode(FileKind::Document, &Document::empty()).unwrap();
    assert_eq!(&bytes[0..4], b"TGF\0");
    assert_eq!(
        u16::from_le_bytes([bytes[4], bytes[5]]),
        storage::TGF_VERSION
    );
}

#[test]
fn newer_tgf_version_is_refused() {
    let mut bytes = encode(FileKind::Document, &Document::empty()).unwrap();
    bytes[4..6].copy_from_slice(&(storage::TGF_VERSION + 1).to_le_bytes());
    let err = decode::<Document>(FileKind::Document, &bytes).unwrap_err();
    assert!(matches!(err, StorageError::UnsupportedVersion(_)));
}

#[test]
fn foreign_files_are_refused() {
    let err = decode::<Document>(FileKind::Document, b"PK\x03\x04 not ours").unwrap_err();
    assert!(matches!(err, StorageError::NotTgf));
    let err = decode::<Document>(FileKind::Document, b"TG").unwrap_err();
    assert!(matches!(err, StorageError::NotTgf));
}

#[test]
fn truncated_tgf_is_reported_as_corrupt() {
    let bytes = encode(FileKind::Document, &doc_with(&["x"])).unwrap();
    let err = decode::<Document>(FileKind::Document, &bytes[..bytes.len() - 3]).unwrap_err();
    assert!(matches!(err, StorageError::Corrupt(_)));
}

#[test]
fn tgf_of_another_kind_is_refused() {
    let bytes = encode(FileKind::Library, &Library::default()).unwrap();
    let err = decode::<Document>(FileKind::Document, &bytes).unwrap_err();
    assert!(matches!(err, StorageError::WrongKind));
}

#[test]
fn write_atomic_creates_parents_and_replaces_contents() {
    let dir = std::env::temp_dir().join(format!("lexgraph-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("nested").join("a.tgf");
    storage::write_atomic(&path, b"one").unwrap();
    storage::write_atomic(&path, b"two").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"two");
    let leftovers = std::fs::read_dir(path.parent().unwrap()).unwrap().count();
    assert_eq!(leftovers, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn json_export_round_trips_saved_expressions_and_settings() {
    let mut settings = Settings {
        transparent_export: true,
        ..Settings::default()
    };
    settings
        .shortcuts
        .rebind(Command::Save, combo(true, true, "k"))
        .unwrap();
    let text = export_json(&library(), &settings).unwrap();
    assert!(text.contains("Parabola"));
    let bundle: ExportBundle = import_json(&text).unwrap();
    assert_eq!(bundle.saved, library().saved);
    assert_eq!(bundle.settings, settings);
}

#[test]
fn json_import_rejects_garbage_and_newer_versions() {
    assert!(matches!(import_json("{nope"), Err(StorageError::Json(_))));
    let text = export_json(&Library::default(), &Settings::default()).unwrap();
    let newer = text.replacen("\"version\": 1", "\"version\": 99", 1);
    assert!(matches!(
        import_json(&newer),
        Err(StorageError::UnsupportedVersion(99))
    ));
}

#[test]
fn import_merge_skips_duplicates_and_keeps_existing() {
    let mut lib = library();
    let incoming = vec![
        lib.saved[0].clone(),
        SavedExpr {
            name: "New".into(),
            source: "x".into(),
            tags: vec![],
            favorite: false,
        },
    ];
    assert_eq!(lib.merge(incoming), 1);
    assert_eq!(lib.saved.len(), 3);
    assert_eq!(lib.saved[2].name, "New");
}

// ---- plot ----

fn plot(doc: &Document) -> lexgraph::app::plot::PlotResult {
    build_plot(doc, &compile(doc), SceneTheme::dark(), 800)
}

fn count(result: &lexgraph::app::plot::PlotResult, kind: MarkerKind) -> usize {
    result
        .spec
        .markers
        .iter()
        .filter(|m| m.kind == kind)
        .count()
}

#[test]
fn plot_marks_roots_and_extrema() {
    let result = plot(&doc_with(&["x^2-4"]));
    assert_eq!(count(&result, MarkerKind::Root), 2);
    assert_eq!(count(&result, MarkerKind::Extremum), 1);
    let root = result
        .spec
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Root)
        .unwrap();
    assert!((root.x + 2.0).abs() < 1e-6);
    assert!(root.label.as_deref().unwrap().contains("-2"));
}

#[test]
fn marker_labels_hide_numeric_noise_relative_to_the_view() {
    let result = plot(&doc_with(&["x^2-4"]));
    let extremum = result
        .spec
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Extremum)
        .unwrap();
    assert_eq!(extremum.label.as_deref(), Some("(0, -4)"));
}

#[test]
fn marker_labels_keep_precision_when_zoomed_in() {
    let mut doc = doc_with(&["x-0.00001"]);
    doc.viewport = Viewport {
        x_min: 0.0,
        x_max: 0.00002,
        y_min: -0.00001,
        y_max: 0.00001,
    };
    let result = plot(&doc);
    let label = result.spec.markers[0].label.clone().unwrap();
    assert!(label.starts_with("(1"), "{label}");
    assert!(label.contains("e-5"), "{label}");
}

#[test]
fn plot_marks_intersections_between_curves() {
    let result = plot(&doc_with(&["x^2", "x+2"]));
    assert_eq!(count(&result, MarkerKind::Intersection), 2);
}

#[test]
fn analysis_points_can_be_turned_off() {
    let mut doc = doc_with(&["x^2-4", "x"]);
    doc.display.show_points = false;
    assert!(plot(&doc).spec.markers.is_empty());
}

#[test]
fn hidden_and_invalid_entries_are_not_drawn() {
    let mut doc = doc_with(&["x", "\\frac{", "x+1"]);
    doc.entries[2].visible = false;
    let result = plot(&doc);
    assert_eq!(result.spec.curves.len(), 1);
    assert_eq!(result.spec.curves[0].label, "x");
}

#[test]
fn compile_keeps_the_error_for_display() {
    let doc = doc_with(&["\\foo"]);
    let compiled = compile(&doc);
    let error = compiled.error_of(doc.entries[0].id).unwrap();
    assert_eq!(error.span, 0..4);
    assert!(error.to_string().contains("\\foo"));
}

#[test]
fn derivative_adds_a_dashed_curve() {
    let mut doc = doc_with(&["x^2"]);
    doc.entries[0].show_derivative = true;
    let result = plot(&doc);
    assert_eq!(result.spec.curves.len(), 2);
    let derivative = &result.spec.curves[1];
    assert_eq!(derivative.style, LineStyle::Dashed);
    assert!(derivative.label.contains('\''));
    // f'(1) = 2
    let near_one = derivative.lines[0]
        .iter()
        .min_by(|a, b| (a.0 - 1.0).abs().total_cmp(&(b.0 - 1.0).abs()))
        .unwrap();
    assert!((near_one.1 - 2.0).abs() < 0.05);
}

#[test]
fn parametric_derivative_plots_slope_against_x() {
    let mut doc = doc_with(&["(t, t^2)"]);
    doc.entries[0].show_derivative = true;
    doc.entries[0].t_min = -1.0;
    doc.entries[0].t_max = 1.0;
    let result = plot(&doc);
    let (x, y) = result.spec.curves[1].lines[0][0];
    assert!((x + 1.0).abs() < 1e-9 && (y + 2.0).abs() < 1e-9, "{x},{y}");
}

#[test]
fn integral_fill_computes_value_and_polygon() {
    let mut doc = doc_with(&["x^2"]);
    let id = doc.entries[0].id;
    doc.entries[0].integral.enabled = true;
    doc.entries[0].integral.a = "0".into();
    doc.entries[0].integral.b = "3".into();
    let result = plot(&doc);
    assert!(result.spec.curves[0].fill.is_some());
    assert!((result.integrals[&id].as_ref().unwrap() - 9.0).abs() < 1e-6);
    assert!(result.spec.curves[0].label.contains('9'));
}

#[test]
fn integral_bounds_may_use_params_and_constants() {
    let mut doc = doc_with(&["\\sin x + 0 c"]);
    let id = doc.entries[0].id;
    doc.params.get_mut("c").unwrap().value = 1.0;
    doc.entries[0].integral.enabled = true;
    doc.entries[0].integral.a = "0".into();
    doc.entries[0].integral.b = "c\\pi".into();
    let result = plot(&doc);
    assert!((result.integrals[&id].as_ref().unwrap() - 2.0).abs() < 1e-6);
}

#[test]
fn invalid_integral_bound_reports_an_error_without_fill() {
    let mut doc = doc_with(&["x"]);
    let id = doc.entries[0].id;
    doc.entries[0].integral.enabled = true;
    doc.entries[0].integral.a = "\\foo".into();
    doc.entries[0].integral.b = "1".into();
    let result = plot(&doc);
    assert!(result.integrals[&id].is_err());
    assert!(result.spec.curves[0].fill.is_none());
}

#[test]
fn slider_value_changes_the_curve() {
    let mut doc = doc_with(&["a"]);
    doc.params.get_mut("a").unwrap().value = 3.0;
    let result = plot(&doc);
    assert!((result.spec.curves[0].lines[0][0].1 - 3.0).abs() < 1e-12);
}

#[test]
fn hover_snaps_to_the_nearest_curve_within_reach() {
    let result = plot(&doc_with(&["0"]));
    // The canvas is 200x200 over [-10,10]^2, so y=0 is at py=100.
    let snapped = snap_to_curve(&result.spec, (100.0, 104.0), 200.0, 200.0, 12.0).unwrap();
    assert!(snapped.y.abs() < 1e-9);
    assert!(snapped.x.abs() < 0.1);
    assert!(snap_to_curve(&result.spec, (100.0, 150.0), 200.0, 200.0, 12.0).is_none());
}

#[test]
fn entry_default_is_a_visible_solid_line() {
    let entry: &ExprEntry = &doc_with(&["x"]).entries[0];
    assert!(entry.visible);
    assert_eq!(entry.style, LineStyle::Solid);
}

// ---- graph copy key and settings migration ----

#[test]
fn graph_copy_defaults_to_ctrl_shift_g() {
    let s = Shortcuts::default();
    assert_eq!(s.lookup(&combo(true, true, "g")), Some(Command::CopyGraph));
    assert_eq!(s.lookup(&combo(true, true, "c")), None);
}

#[derive(serde::Serialize)]
struct SettingsV1 {
    shortcuts: Shortcuts,
    transparent_export: bool,
}

fn settings_v1_bytes(shortcuts: Shortcuts) -> Vec<u8> {
    let old = SettingsV1 {
        shortcuts,
        transparent_export: true,
    };
    let mut bytes = b"TGF\0".to_vec();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.push(FileKind::Settings as u8);
    bytes.extend_from_slice(&postcard::to_stdvec(&old).unwrap());
    bytes
}

fn old_default_shortcuts() -> Shortcuts {
    let mut s = Shortcuts::default();
    s.rebind(Command::CopyGraph, combo(true, true, "c"))
        .unwrap();
    s
}

#[test]
fn v1_settings_keep_their_values_and_get_the_default_png_size() {
    let mut shortcuts = old_default_shortcuts();
    shortcuts
        .rebind(Command::Save, combo(true, true, "k"))
        .unwrap();
    let settings = decode_settings(&settings_v1_bytes(shortcuts)).unwrap();
    assert!(settings.transparent_export);
    assert_eq!(settings.png_size, PngSize::default());
    assert_eq!(
        settings.shortcuts.lookup(&combo(true, true, "k")),
        Some(Command::Save)
    );
}

#[test]
fn v1_settings_move_the_old_copy_key_to_the_new_default() {
    let settings = decode_settings(&settings_v1_bytes(old_default_shortcuts())).unwrap();
    assert_eq!(
        settings.shortcuts.lookup(&combo(true, true, "g")),
        Some(Command::CopyGraph)
    );
    assert_eq!(settings.shortcuts.lookup(&combo(true, true, "c")), None);
}

#[test]
fn v1_migration_keeps_a_copy_key_the_user_chose() {
    let mut shortcuts = old_default_shortcuts();
    shortcuts
        .rebind(Command::CopyGraph, combo(true, true, "j"))
        .unwrap();
    let settings = decode_settings(&settings_v1_bytes(shortcuts)).unwrap();
    assert_eq!(
        settings.shortcuts.lookup(&combo(true, true, "j")),
        Some(Command::CopyGraph)
    );
}

#[test]
fn v1_migration_does_not_steal_a_key_in_use() {
    let mut shortcuts = old_default_shortcuts();
    shortcuts
        .rebind(Command::Save, combo(true, true, "g"))
        .unwrap();
    let settings = decode_settings(&settings_v1_bytes(shortcuts)).unwrap();
    assert_eq!(
        settings.shortcuts.lookup(&combo(true, true, "g")),
        Some(Command::Save)
    );
    assert_eq!(
        settings.shortcuts.lookup(&combo(true, true, "c")),
        Some(Command::CopyGraph)
    );
}

#[test]
fn current_settings_round_trip_with_the_png_size() {
    let settings = Settings {
        png_size: PngSize::Large,
        ..Settings::default()
    };
    let bytes = encode(FileKind::Settings, &settings).unwrap();
    assert_eq!(decode_settings(&bytes).unwrap(), settings);
}

#[test]
fn documents_saved_by_version_1_still_open() {
    let doc = doc_with(&["x^2"]);
    let mut bytes = encode(FileKind::Document, &doc).unwrap();
    bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(decode::<Document>(FileKind::Document, &bytes).unwrap(), doc);
}

#[test]
fn json_exported_before_png_size_still_imports() {
    let text = export_json(&Library::default(), &Settings::default()).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["settings"]
        .as_object_mut()
        .unwrap()
        .remove("png_size");
    let bundle = import_json(&value.to_string()).unwrap();
    assert_eq!(bundle.settings.png_size, PngSize::default());
}

// ---- undo steps ----

#[test]
fn releasing_a_slider_ends_its_undo_step() {
    let mut h = History::new(10);
    h.record_coalesced(1, "s0");
    h.record_coalesced(1, "s1");
    h.end_coalescing();
    h.record_coalesced(1, "s2");
    assert_eq!(h.undo("s3"), Some("s2"));
    assert_eq!(h.undo("s2"), Some("s0"));
}

#[test]
fn every_edited_field_has_its_own_undo_key() {
    let keys = [
        EditField::Source(1).key(),
        EditField::Source(2).key(),
        EditField::Width(1).key(),
        EditField::IntegralBound(1, true).key(),
        EditField::IntegralBound(1, false).key(),
        EditField::CurveRange(1, true).key(),
        EditField::CurveRange(1, false).key(),
        EditField::Param("a", 0).key(),
        EditField::Param("a", 1).key(),
        EditField::Param("b", 0).key(),
    ];
    let distinct: std::collections::HashSet<u64> = keys.iter().copied().collect();
    assert_eq!(distinct.len(), keys.len());
}

#[test]
fn an_undo_key_stays_the_same_while_editing_one_field() {
    assert_eq!(
        EditField::Param("a", 0).key(),
        EditField::Param("a", 0).key()
    );
    assert_eq!(EditField::Source(7).key(), EditField::Source(7).key());
}
