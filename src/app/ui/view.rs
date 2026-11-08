//! Screen layout. The left pane holds only the expression list; the right holds the graph.
//!
//! Compact density for mouse and keyboard: spacing comes from a 4 / 8 / 12 scale,
//! text from a 12 / 13 / 15 / 18 scale.
use super::graph::Graph;
use super::{
    App, DisplayField, Message, Overlay, PaneKind, ParamField, editor_id, key_combo, style,
};
use crate::app::model::{Command, ExprEntry, PALETTE, ParamSetting, PngSize, Shortcuts};
use crate::app::output::{describe_error, rgb_to_hex};
use crate::app::palette::CATEGORIES;
use crate::app::plot::short_number;
use crate::app::theme::{ERROR, INK_MUTED};
use crate::core::FormulaKind;
use crate::render::canvas::color as to_color;
use crate::render::{LineStyle, format_tick};
use iced::font::Weight;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::widget::text_editor::{Binding, KeyPress};
use iced::widget::{
    Column, Row, Tooltip, button, canvas, center, checkbox, column, container, mouse_area, opaque,
    pane_grid, pick_list, row, rule, scrollable, slider, space, stack, text, text_editor,
    text_input, tooltip,
};
use iced::{Alignment, Color, Element, Fill, Font, Length, Padding};
use std::collections::BTreeSet;

const TEX_FONT: Font = Font::with_name("Consolas");
const STRONG: Font = Font {
    weight: Weight::Semibold,
    ..Font::with_name("Segoe UI")
};
const BODY: u32 = 13;
const SMALL: u32 = 12;
const SCROLL_GUTTER: Padding = Padding {
    right: 12.0,
    ..Padding::ZERO
};

fn muted() -> Color {
    to_color(INK_MUTED)
}

fn error_color() -> Color {
    to_color(ERROR)
}

pub fn hex_draft_key(id: u64) -> String {
    format!("hex:{id}")
}

pub fn curve_draft_key(id: u64, is_min: bool) -> String {
    format!("curve:{id}:{is_min}")
}

pub fn param_draft_key(name: &str, field: ParamField) -> String {
    format!("param:{name}:{field:?}")
}

pub const SAVED_TAGS_DRAFT_PREFIX: &str = "saved-tags:";

pub fn saved_tags_draft_key(index: usize) -> String {
    format!("{SAVED_TAGS_DRAFT_PREFIX}{index}")
}

fn range_draft_key(index: usize) -> String {
    format!("range:{index}")
}

fn caption(content: impl text::IntoFragment<'static>) -> iced::widget::Text<'static> {
    text(content).size(SMALL).color(muted())
}

fn heading(content: &'static str) -> iced::widget::Text<'static> {
    text(content).size(BODY + 1).font(STRONG)
}

/// Explains a control on hover; the wording says what it does, not what it is.
fn with_tip<'a>(
    content: impl Into<Element<'a, Message>>,
    tip: impl text::IntoFragment<'a>,
) -> Tooltip<'a, Message> {
    tooltip(
        content,
        container(text(tip).size(SMALL))
            .padding([4, 8])
            .style(style::tooltip_box),
        tooltip::Position::Bottom,
    )
    .gap(4)
}

pub fn root(app: &App) -> Element<'_, Message> {
    let panes = pane_grid::PaneGrid::new(&app.panes, |_pane, kind, _maximized| {
        pane_grid::Content::new(match kind {
            PaneKind::Expressions => expression_pane(app),
            PaneKind::Graph => graph_pane(app),
        })
    })
    .spacing(4)
    .min_size(260)
    .on_resize(10, Message::PaneResized)
    .style(style::panes);

    let base = column![
        toolbar(app),
        container(panes).height(Fill).style(style::ground),
        status_line(app)
    ];

    let Some(overlay) = app.overlay else {
        return base.into();
    };
    let content = match overlay {
        Overlay::Library => library_panel(app),
        Overlay::Settings => settings_panel(app),
    };
    let modal = container(content)
        .width(760)
        .max_height(720)
        .padding(16)
        .style(style::dialog);
    let backdrop =
        mouse_area(center(opaque(modal)).style(style::backdrop)).on_press(Message::CloseOverlay);
    stack![base, opaque(backdrop)].into()
}

fn command_tip(app: &App, command: Command, extra: Option<&str>) -> String {
    let key = app
        .settings
        .shortcuts
        .bindings
        .iter()
        .find(|(c, _)| *c == command)
        .map(|(_, combo)| combo.label());
    let mut tip = command.title().to_string();
    if let Some(key) = key {
        tip.push_str(&format!("（{key}）"));
    }
    if let Some(extra) = extra {
        tip.push_str(&format!(" — {extra}"));
    }
    tip
}

fn tool(label: &'static str, command: Command, app: &App) -> Element<'static, Message> {
    tool_when(label, command, app, true, None)
}

/// A toolbar button. When it can't run, the tooltip says why.
fn tool_when(
    label: &'static str,
    command: Command,
    app: &App,
    enabled: bool,
    why_not: Option<&str>,
) -> Element<'static, Message> {
    let extra = if enabled { None } else { why_not };
    let tip = command_tip(app, command, extra);
    with_tip(
        button(text(label).size(BODY))
            .padding([4, 10])
            .style(style::tool_button)
            .on_press_maybe(enabled.then_some(Message::Command(command))),
        tip,
    )
    .into()
}

fn divider() -> Element<'static, Message> {
    container(rule::vertical(1)).height(18).into()
}

fn toolbar(app: &App) -> Element<'_, Message> {
    let bar = row![
        tool("新規", Command::New, app),
        tool("開く", Command::Open, app),
        tool("保存", Command::Save, app),
        divider(),
        tool_when(
            "取り消し",
            Command::Undo,
            app,
            app.undo.can_undo(),
            Some("取り消せる操作はありません"),
        ),
        tool_when(
            "やり直し",
            Command::Redo,
            app,
            app.undo.can_redo(),
            Some("やり直せる操作はありません"),
        ),
        divider(),
        tool("式ライブラリ", Command::Library, app),
        divider(),
        tool("書き出し", Command::Export, app),
        tool("コピー", Command::CopyGraph, app),
        checkbox(app.settings.transparent_export)
            .label("背景を透過")
            .text_size(BODY)
            .size(14)
            .on_toggle(Message::TransparentToggled),
        space().width(4),
        with_tip(
            row![
                caption("PNG サイズ"),
                pick_list(
                    &PngSize::ALL[..],
                    Some(app.settings.png_size),
                    Message::PngSizeChanged
                )
                .text_size(BODY)
                .padding([3, 8]),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            "PNG の保存とコピーの大きさ",
        ),
        space().width(Fill),
        button(text("設定").size(BODY))
            .padding([4, 10])
            .style(style::tool_button)
            .on_press(Message::OpenOverlay(Overlay::Settings)),
    ]
    .spacing(4)
    .padding([4, 8])
    .align_y(Alignment::Center);
    container(bar).width(Fill).style(style::shell).into()
}

/// One line that always keeps its height. A symbol accompanies the color.
fn status_line(app: &App) -> Element<'_, Message> {
    let line = match &app.status {
        Some((message, true)) => text(format!("⚠ {message}"))
            .size(SMALL)
            .color(error_color()),
        Some((message, false)) => text(format!("✓ {message}")).size(SMALL).color(muted()),
        None => text(" ").size(SMALL),
    };
    container(line)
        .padding([3, 12])
        .width(Fill)
        .style(style::shell)
        .into()
}

fn expression_pane(app: &App) -> Element<'_, Message> {
    let mut shown_params = BTreeSet::new();
    let mut list = Column::new().spacing(8).padding(SCROLL_GUTTER);
    for entry in &app.doc.entries {
        list = list.push(expression_card(app, entry, &mut shown_params));
    }
    list = list.push(with_tip(
        button(text("＋ 式を追加").size(BODY))
            .padding([4, 10])
            .style(style::tool_button)
            .on_press(Message::Command(Command::AddExpression)),
        command_tip(app, Command::AddExpression, None),
    ));
    container(scrollable(list).height(Fill))
        .padding(8)
        .height(Fill)
        .style(style::ground)
        .into()
}

fn swatch(color: crate::render::Rgb, size: f32) -> iced::widget::Button<'static, Message> {
    button(space().width(size).height(size))
        .padding(0)
        .style(style::swatch_button(color))
}

/// Keys assigned to a shortcut must not also act on the text field: its built-in
/// Ctrl+C would, for one, overwrite a graph just copied to the clipboard.
fn tex_key_binding(id: u64, shortcuts: Shortcuts) -> impl Fn(KeyPress) -> Option<Binding<Message>> {
    move |press: KeyPress| {
        let combo = key_combo(&press.key, press.physical_key, press.modifiers);
        if combo.is_some_and(|c| shortcuts.lookup(&c).is_some()) {
            return None;
        }
        if press.key == Key::Named(Named::Enter) {
            return Some(Binding::Custom(Message::EditorEnter(id)));
        }
        Binding::from_key_press(press)
    }
}

fn expression_card<'a>(
    app: &'a App,
    entry: &'a ExprEntry,
    shown_params: &mut BTreeSet<String>,
) -> Element<'a, Message> {
    let id = entry.id;
    let is_active = app.active == Some(id);
    let is_expanded = app.expanded.contains(&id);
    let error = app.visible_error(id);

    let mut input = Column::new();
    if let Some(content) = app.editors.get(&id) {
        input = input.push(
            text_editor(content)
                .id(editor_id(id))
                .placeholder("例: \\frac{1}{2}x^2 + \\sin x")
                .font(TEX_FONT)
                .size(15)
                .height(Length::Shrink)
                .style(style::tex_editor(error.is_some()))
                .on_action(move |action| Message::Editor(id, action))
                .key_binding(tex_key_binding(id, app.settings.shortcuts.clone())),
        );
    }

    let details_style = if is_expanded {
        style::control_button
    } else {
        style::tool_button
    };
    let header = row![
        with_tip(
            checkbox(entry.visible)
                .size(14)
                .on_toggle(move |on| Message::VisibleToggled(id, on)),
            "グラフに表示",
        ),
        container(input).width(Fill),
        with_tip(
            button(text("詳細").size(SMALL))
                .padding([3, 8])
                .style(details_style)
                .on_press(Message::ToggleExpanded(id)),
            "色・線・導関数・定積分",
        ),
        with_tip(
            button(text("×").size(BODY))
                .padding([2, 8])
                .style(style::tool_button)
                .on_press(Message::Remove(id)),
            "この式を削除（Ctrl+Z で戻せます）",
        ),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let mut body = Column::new().spacing(8).push(header);

    // The palette stays directly under the input. Errors go below it, so the keys
    // don't jump while typing.
    if is_active {
        body = body.push(palette(app));
    }
    if let Some(error) = error {
        body = body.push(
            text(format!("⚠ {}", describe_error(&entry.source, error)))
                .size(BODY)
                .color(error_color()),
        );
    }
    if let Some(formula) = app.compiled.formula_of(id) {
        let names: Vec<&String> = formula
            .params
            .iter()
            .filter(|name| shown_params.insert((*name).clone()))
            .collect();
        for name in names {
            if let Some(setting) = app.doc.params.get(name) {
                body = body.push(param_row(app, name, *setting));
            }
        }
    }
    for name in entry.bound_params() {
        if shown_params.insert(name.clone())
            && let Some(setting) = app.doc.params.get(&name)
        {
            body = body.push(param_row(app, &name, *setting));
        }
    }
    if is_expanded {
        body = body.push(options(app, entry));
    }
    // Below the options: turning the integral on must not move its checkbox from under the cursor.
    if let Some(result) = app.plot.integrals.get(&id) {
        body = body.push(match result {
            Ok(value) => text(format!("∫ = {}", short_number(*value))).size(BODY),
            Err(reason) => text(format!("⚠ {reason}")).size(BODY).color(error_color()),
        });
    }

    // The bar in the curve's color ties each expression to its line in the graph.
    let bar = container(space())
        .width(4)
        .height(Fill)
        .style(style::curve_bar(entry.color, entry.visible));
    let content = row![bar, container(body).width(Fill)]
        .spacing(8)
        .height(Length::Shrink);

    container(content)
        .padding(8)
        .width(Fill)
        .style(style::card(is_active))
        .into()
}

fn palette(app: &App) -> Element<'_, Message> {
    let tabs =
        CATEGORIES
            .iter()
            .enumerate()
            .fold(Row::new().spacing(4), |tabs, (index, category)| {
                let tab_style = if index == app.palette_category {
                    style::primary_button
                } else {
                    style::tool_button
                };
                tabs.push(
                    button(text(category.name).size(SMALL))
                        .padding([2, 6])
                        .style(tab_style)
                        .on_press(Message::PaletteCategory(index)),
                )
            });
    let category = app.palette_category.min(CATEGORIES.len() - 1);
    let keys = CATEGORIES[category].items.iter().enumerate().fold(
        Row::new().spacing(4),
        |keys, (index, item)| {
            // Japanese fonts draw a backslash as a yen sign, so TeX names need the code font.
            let font = if item.label.starts_with('\\') {
                TEX_FONT
            } else {
                Font::default()
            };
            keys.push(
                button(text(item.label).size(SMALL).font(font))
                    .padding([3, 8])
                    .style(style::control_button)
                    .on_press(Message::PaletteInsert(category, index)),
            )
        },
    );
    column![tabs.wrap(), keys.wrap().vertical_spacing(4)]
        .spacing(4)
        .into()
}

fn small_field<'a>(
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
) -> iced::widget::TextInput<'a, Message> {
    text_input(placeholder, value)
        .size(SMALL)
        .padding([3, 6])
        .width(58)
        .style(style::field)
        .on_input(on_input)
}

fn draft_or(app: &App, key: &str, fallback: String) -> String {
    app.drafts.get(key).cloned().unwrap_or(fallback)
}

fn param_row<'a>(app: &'a App, name: &str, setting: ParamSetting) -> Element<'a, Message> {
    let high = if setting.max > setting.min {
        setting.max
    } else {
        setting.min + 1.0
    };
    let step = (high - setting.min) / 1000.0;
    let on_slide = {
        let name = name.to_string();
        move |value| Message::ParamChanged(name.clone(), value)
    };
    let field = |kind: ParamField, current: f64| {
        let owned = name.to_string();
        small_field(
            "",
            &draft_or(app, &param_draft_key(name, kind), format_tick(current)),
            move |text| Message::ParamEdited(owned.clone(), kind, text),
        )
    };
    row![
        text(format!("{} =", name.trim_start_matches('\\')))
            .size(BODY)
            .font(TEX_FONT),
        field(ParamField::Value, setting.value),
        space().width(4),
        with_tip(field(ParamField::Min, setting.min), "スライダーの最小値"),
        slider(
            setting.min..=high,
            setting.value.clamp(setting.min, high),
            on_slide
        )
        .on_release(Message::SliderReleased)
        .step(step)
        .width(Fill),
        with_tip(field(ParamField::Max, setting.max), "スライダーの最大値"),
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .into()
}

fn options<'a>(app: &'a App, entry: &'a ExprEntry) -> Element<'a, Message> {
    let id = entry.id;
    let swatches = PALETTE.iter().fold(Row::new().spacing(4), |row, color| {
        row.push(swatch(*color, 16.0).on_press(Message::ColorPicked(id, *color)))
    });
    let hex = text_input(
        "#rrggbb",
        &draft_or(app, &hex_draft_key(id), rgb_to_hex(entry.color)),
    )
    .size(SMALL)
    .padding([3, 6])
    .width(74)
    .style(style::field)
    .on_input(move |value| Message::HexEdited(id, value));

    let line = row![
        caption("太さ"),
        slider(0.5f32..=8.0, entry.width, move |w| Message::WidthChanged(
            id, w
        ))
        .on_release(Message::SliderReleased)
        .step(0.1f32)
        .width(Fill),
        caption("線種"),
        pick_list(&LineStyle::ALL[..], Some(entry.style), move |style| {
            Message::StyleChanged(id, style)
        })
        .text_size(SMALL)
        .padding([3, 8]),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let kind = app.compiled.formula_of(id).map(|f| &f.kind);
    let mut col = column![
        rule::horizontal(1),
        row![caption("色"), swatches, hex]
            .spacing(8)
            .align_y(Alignment::Center),
        line,
        checkbox(entry.show_derivative)
            .label("導関数を表示")
            .text_size(BODY)
            .size(14)
            .on_toggle(move |on| Message::DerivativeToggled(id, on)),
    ]
    .spacing(8);

    if matches!(kind, Some(FormulaKind::Cartesian(_))) {
        let bound = |value: &str, is_a: bool| {
            small_field("", value, move |t| Message::BoundEdited(id, is_a, t))
        };
        col = col.push(
            row![
                checkbox(entry.integral.enabled)
                    .label("定積分を塗る")
                    .text_size(BODY)
                    .size(14)
                    .on_toggle(move |on| Message::IntegralToggled(id, on)),
                space().width(4),
                bound(&entry.integral.a, true),
                caption("から"),
                bound(&entry.integral.b, false),
                caption("まで"),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        );
    }
    if matches!(
        kind,
        Some(FormulaKind::Parametric { .. } | FormulaKind::Polar(_))
    ) {
        let bound = |value: f64, is_min: bool| {
            small_field(
                "",
                &draft_or(app, &curve_draft_key(id, is_min), short_number(value)),
                move |text| Message::CurveRangeEdited(id, is_min, text),
            )
        };
        let name = match kind {
            Some(FormulaKind::Polar(_)) => "θ の範囲",
            _ => "t の範囲",
        };
        col = col.push(
            row![
                caption(name),
                bound(entry.t_min, true),
                caption("〜"),
                bound(entry.t_max, false),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        );
    }
    col = col.push(
        button(text("この式をライブラリに保存…").size(SMALL))
            .padding([3, 8])
            .style(style::control_button)
            .on_press(Message::OpenOverlay(Overlay::Library)),
    );
    col.into()
}

fn graph_pane(app: &App) -> Element<'_, Message> {
    let display = app.doc.display;
    let toggle = |label: &'static str, on: bool, field: DisplayField| {
        checkbox(on)
            .label(label)
            .text_size(SMALL)
            .size(14)
            .on_toggle(move |value| Message::DisplayToggled(field, value))
    };
    let vp = app.doc.viewport;
    let range_field = |index: usize, value: f64| {
        text_input(
            "",
            &draft_or(app, &range_draft_key(index), short_number(value)),
        )
        .size(SMALL)
        .padding([3, 6])
        .width(62)
        .style(style::field)
        .on_input(move |text| Message::RangeEdited(index, text))
    };
    let toggles = row![
        toggle("凡例", display.show_legend, DisplayField::Legend),
        toggle("グリッド", display.show_grid, DisplayField::Grid),
        toggle("目盛り", display.show_ticks, DisplayField::Ticks),
        toggle(
            "軸ラベル",
            display.show_axis_labels,
            DisplayField::AxisLabels
        ),
        toggle(
            "交点・極値・零点",
            display.show_points,
            DisplayField::Points
        ),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let ranges = row![
        caption("x"),
        range_field(0, vp.x_min),
        caption("〜"),
        range_field(1, vp.x_max),
        space().width(8),
        caption("y"),
        range_field(2, vp.y_min),
        caption("〜"),
        range_field(3, vp.y_max),
        space().width(8),
        checkbox(app.doc.auto_fit)
            .label("自動範囲")
            .text_size(SMALL)
            .size(14)
            .on_toggle(Message::AutoFitToggled),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    let strip = row![toggles, space().width(Fill), ranges]
        .spacing(12)
        .padding([4, 8])
        .align_y(Alignment::Center)
        .wrap()
        .vertical_spacing(4);

    let graph = canvas(Graph {
        spec: &app.plot.spec,
        cache: &app.cache,
        hover: app.hover,
    })
    .width(Fill)
    .height(Fill);

    column![container(strip).width(Fill).style(style::ground), graph].into()
}

fn library_panel(app: &App) -> Element<'_, Message> {
    let save_row = row![
        text_input("名前（省略可）", &app.save_name)
            .size(BODY)
            .padding([4, 8])
            .style(style::field)
            .on_input(Message::SaveNameEdited),
        text_input("タグ（カンマ区切り）", &app.save_tags)
            .size(BODY)
            .padding([4, 8])
            .style(style::field)
            .on_input(Message::SaveTagsEdited),
        button(text("選択中の式を保存").size(BODY))
            .padding([4, 10])
            .style(style::primary_button)
            .on_press(Message::SaveCurrent),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let search_row = row![
        text_input("名前・式・タグで検索", &app.library_query)
            .size(BODY)
            .padding([4, 8])
            .style(style::field)
            .on_input(Message::LibraryQuery),
        checkbox(app.library_favorites_only)
            .label("お気に入りのみ")
            .text_size(BODY)
            .size(14)
            .on_toggle(Message::LibraryFavoritesOnly),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let matches = app
        .library
        .search(&app.library_query, app.library_favorites_only);
    let mut saved = Column::new().spacing(4);
    if matches.is_empty() {
        let is_filtered = !app.library.saved.is_empty();
        saved = saved.push(caption(if is_filtered {
            "条件に合う式はありません。検索語を変えるか、絞り込みを外してください。"
        } else {
            "保存した式はまだありません。上の欄から選択中の式を保存すると、ここから呼び出せます。"
        }));
    }
    for index in matches {
        let item = &app.library.saved[index];
        let star = if item.favorite { "★" } else { "☆" };
        saved = saved.push(
            row![
                with_tip(
                    button(text(star).size(BODY))
                        .padding([2, 6])
                        .style(style::tool_button)
                        .on_press(Message::SavedFavorite(index)),
                    "お気に入りの切替",
                ),
                text_input("名前", &item.name)
                    .size(SMALL)
                    .padding([3, 6])
                    .width(150)
                    .style(style::field)
                    .on_input(move |t| Message::SavedNameEdited(index, t)),
                text(item.source.clone())
                    .size(BODY)
                    .font(TEX_FONT)
                    .width(Fill),
                text_input(
                    "タグ",
                    &draft_or(app, &saved_tags_draft_key(index), item.tags.join(", "))
                )
                .size(SMALL)
                .padding([3, 6])
                .width(130)
                .style(style::field)
                .on_input(move |t| Message::SavedTagsEdited(index, t)),
                button(text("追加").size(SMALL))
                    .padding([3, 8])
                    .style(style::control_button)
                    .on_press(Message::SavedAdd(index)),
                with_tip(
                    button(text("×").size(BODY))
                        .padding([2, 8])
                        .style(style::tool_button)
                        .on_press(Message::SavedDelete(index)),
                    "保存した式を削除",
                ),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        );
    }

    let mut history = Column::new().spacing(4);
    if app.library.history.is_empty() {
        history = history.push(caption(
            "履歴はまだありません。正しく入力できた式がここに残ります。",
        ));
    }
    for (index, source) in app.library.history.iter().enumerate() {
        history = history.push(
            row![
                text(source.clone()).size(BODY).font(TEX_FONT).width(Fill),
                button(text("追加").size(SMALL))
                    .padding([3, 8])
                    .style(style::control_button)
                    .on_press(Message::HistoryAdd(index)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }

    column![
        panel_header("式ライブラリ"),
        save_row,
        search_row,
        rule::horizontal(1),
        scrollable(
            column![
                heading("保存した式"),
                saved,
                space().height(8),
                heading("履歴"),
                history,
            ]
            .spacing(8)
            .padding(SCROLL_GUTTER)
        )
        .height(Length::Fill),
    ]
    .spacing(12)
    .into()
}

fn settings_panel(app: &App) -> Element<'_, Message> {
    let mut list = Column::new().spacing(4);
    for (command, combo) in &app.settings.shortcuts.bindings {
        let is_capturing = app.capturing == Some(*command);
        let key_text = if is_capturing {
            text("キーを押してください（Esc で中止）")
                .size(BODY)
                .color(to_color(crate::app::theme::ACCENT))
        } else {
            text(combo.label()).size(BODY)
        };
        list = list.push(
            row![
                text(command.title()).size(BODY).width(180),
                container(key_text).width(Fill),
                button(text("変更").size(SMALL))
                    .padding([3, 8])
                    .style(style::control_button)
                    .on_press(Message::CaptureShortcut(*command)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    column![
        panel_header("設定"),
        heading("保存式と設定のエクスポート／インポート（JSON）"),
        row![
            button(text("エクスポート").size(BODY))
                .padding([4, 10])
                .style(style::control_button)
                .on_press(Message::JsonExport),
            button(text("インポート").size(BODY))
                .padding([4, 10])
                .style(style::control_button)
                .on_press(Message::JsonImport),
        ]
        .spacing(8),
        rule::horizontal(1),
        row![
            heading("ショートカット").width(Fill),
            button(text("初期状態に戻す").size(SMALL))
                .padding([3, 8])
                .style(style::control_button)
                .on_press(Message::ResetShortcuts),
        ]
        .align_y(Alignment::Center),
        scrollable(list.padding(SCROLL_GUTTER)).height(Length::Fill),
    ]
    .spacing(12)
    .into()
}

fn panel_header(title: &'static str) -> Element<'static, Message> {
    row![
        text(title).size(18).font(STRONG).width(Fill),
        button(text("閉じる").size(BODY))
            .padding([4, 10])
            .style(style::control_button)
            .on_press(Message::CloseOverlay),
    ]
    .align_y(Alignment::Center)
    .into()
}
