//! The pictures and lists of the palettes (docs/design/visual-redesign.md
//! section 7): symbols drawn as the real IEC 60617 artwork, the category
//! column, the card grid and the detail pane of the place palette (Q), and
//! the row pieces the command palette (Ctrl K) shares with it.
//!
//! The search and its grammar stay in `novo-workflow`'s `Palette`; this file
//! only decides what the rows look like and which of them a category shows.

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use novo_parts::Catalog;
use novo_symbols::{Align, Graphic, Library, Stroke as Line, Symbol};
use novo_theme::drawn::Icon;
use novo_theme::presentation::Glyph;
use novo_workflow::palette::{Kind, Row, Sources, Target};

use crate::theme::ui_palette;

// ---------------------------------------------------------------------------
// Symbol artwork

/// One drawn piece of a symbol, in the symbol's own millimetres.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Piece {
    Line {
        a: [f32; 2],
        b: [f32; 2],
        dashed: bool,
    },
    Poly {
        points: Vec<[f32; 2]>,
        closed: bool,
        fill: bool,
        dashed: bool,
    },
    Text {
        at: [f32; 2],
        text: String,
        size: f32,
        align: Align,
    },
}

fn pt(p: [f64; 2]) -> [f32; 2] {
    [p[0] as f32, p[1] as f32]
}

fn arc_points(c: [f64; 2], rx: f64, ry: f64, from: f64, to: f64, steps: usize) -> Vec<[f32; 2]> {
    (0..=steps)
        .map(|i| {
            let a = (from + (to - from) * i as f64 / steps as f64).to_radians();
            [(c[0] + rx * a.cos()) as f32, (c[1] + ry * a.sin()) as f32]
        })
        .collect()
}

/// The artwork of a library symbol as lines, polygons and text. Circles,
/// ellipses and arcs become short segments, so the painter needs one shape.
pub(crate) fn pieces(symbol: &Symbol) -> Vec<Piece> {
    let dashed = |s: &Line| *s == Line::Dashed;
    symbol
        .graphics
        .iter()
        .map(|g| match g {
            Graphic::Line { from, to, stroke } => Piece::Line {
                a: pt(*from),
                b: pt(*to),
                dashed: dashed(stroke),
            },
            Graphic::Polyline {
                points,
                closed,
                fill,
                stroke,
            } => Piece::Poly {
                points: points.iter().map(|p| pt(*p)).collect(),
                closed: *closed || *fill,
                fill: *fill,
                dashed: dashed(stroke),
            },
            Graphic::Rect {
                at,
                size,
                fill,
                stroke,
            } => Piece::Poly {
                points: vec![
                    pt(*at),
                    pt([at[0] + size[0], at[1]]),
                    pt([at[0] + size[0], at[1] + size[1]]),
                    pt([at[0], at[1] + size[1]]),
                ],
                closed: true,
                fill: *fill,
                dashed: dashed(stroke),
            },
            Graphic::Circle {
                center,
                r,
                fill,
                stroke,
            } => Piece::Poly {
                points: arc_points(*center, *r, *r, 0.0, 360.0, 36),
                closed: true,
                fill: *fill,
                dashed: dashed(stroke),
            },
            Graphic::Ellipse {
                center,
                rx,
                ry,
                fill,
                stroke,
            } => Piece::Poly {
                points: arc_points(*center, *rx, *ry, 0.0, 360.0, 36),
                closed: true,
                fill: *fill,
                dashed: dashed(stroke),
            },
            Graphic::Arc {
                center,
                r,
                start,
                end,
                stroke,
            } => Piece::Poly {
                points: arc_points(*center, *r, *r, *start, *end, 24),
                closed: false,
                fill: false,
                dashed: dashed(stroke),
            },
            Graphic::Text {
                at,
                text,
                size,
                align,
            } => Piece::Text {
                at: pt(*at),
                text: text.clone(),
                size: *size as f32,
                align: *align,
            },
        })
        .collect()
}

/// Largest side of the symbol (artwork and connection points) in mm, at
/// least 4 so a thin contact is not blown up.
pub(crate) fn extent(symbol: &Symbol) -> (f32, f32) {
    let (lo, hi) = symbol.bounds();
    (
        ((hi[0] - lo[0]) as f32).max(4.0),
        ((hi[1] - lo[1]) as f32).max(4.0),
    )
}

/// Points per mm that fit the symbol into `size` with `margin`.
pub(crate) fn fit_scale(symbol: &Symbol, size: Vec2, margin: f32) -> f32 {
    let (w, h) = extent(symbol);
    ((size.x - 2.0 * margin) / w)
        .min((size.y - 2.0 * margin) / h)
        .max(0.01)
}

/// The scale a grid of symbols is drawn at, so a contact and a motor keep
/// their relative size: the fit scale that 80 percent of the symbols can use
/// (the largest fifth is shrunk to fit on its own), capped so a tiny symbol
/// is not blown up.
pub(crate) fn common_scale<'a>(
    symbols: impl IntoIterator<Item = &'a Symbol>,
    cell: Vec2,
    margin: f32,
) -> f32 {
    let mut fits: Vec<f32> = symbols
        .into_iter()
        .map(|s| fit_scale(s, cell, margin))
        .collect();
    if fits.is_empty() {
        return 1.0;
    }
    fits.sort_by(f32::total_cmp);
    fits[fits.len() / 5].min(MAX_SCALE)
}

const MAX_SCALE: f32 = 8.0;

/// Paints `symbol` centred in `rect`. `scale` is points per mm; the symbol
/// is shrunk to fit when it would overflow. `pins` adds the connection
/// points, with their designations when `labels` is set.
pub(crate) fn paint_symbol(
    painter: &Painter,
    rect: Rect,
    symbol: &Symbol,
    scale: Option<f32>,
    color: Color32,
    pins: bool,
    labels: bool,
) {
    let (lo, hi) = symbol.bounds();
    let fit = fit_scale(symbol, rect.size(), 2.0);
    let scale = scale.map_or(fit, |s| s.min(fit));
    let centre = Pos2::new(
        ((lo[0] + hi[0]) / 2.0) as f32,
        ((lo[1] + hi[1]) / 2.0) as f32,
    );
    let map = |p: [f32; 2]| {
        Pos2::new(
            rect.center().x + (p[0] - centre.x) * scale,
            rect.center().y + (p[1] - centre.y) * scale,
        )
    };
    let width = (0.3 * scale).clamp(1.0, 2.2);
    let stroke = Stroke::new(width, color);
    for piece in pieces(symbol) {
        match piece {
            Piece::Line { a, b, dashed } => {
                if dashed {
                    painter.extend(egui::Shape::dashed_line(
                        &[map(a), map(b)],
                        stroke,
                        3.0,
                        2.0,
                    ));
                } else {
                    painter.line_segment([map(a), map(b)], stroke);
                }
            }
            Piece::Poly {
                points,
                closed,
                fill,
                dashed,
            } => {
                let points: Vec<Pos2> = points.into_iter().map(map).collect();
                if fill {
                    painter.add(egui::Shape::convex_polygon(
                        points,
                        color,
                        Stroke::new(width, color),
                    ));
                } else if dashed {
                    let mut path = points;
                    if closed && let Some(first) = path.first().copied() {
                        path.push(first);
                    }
                    painter.extend(egui::Shape::dashed_line(&path, stroke, 3.0, 2.0));
                } else if closed {
                    painter.add(egui::Shape::closed_line(points, stroke));
                } else {
                    painter.add(egui::Shape::line(points, stroke));
                }
            }
            Piece::Text {
                at,
                text,
                size,
                align,
            } => {
                let px = size * scale;
                if px >= 5.0 {
                    let anchor = match align {
                        Align::Left => Align2::LEFT_CENTER,
                        Align::Center => Align2::CENTER_CENTER,
                        Align::Right => Align2::RIGHT_CENTER,
                    };
                    painter.text(map(at), anchor, text, FontId::proportional(px), color);
                }
            }
        }
    }
    if pins {
        let p = ui_palette();
        for c in &symbol.connections {
            let at = map(pt(c.at));
            painter.circle_filled(at, (0.5 * scale).clamp(1.5, 3.0), p.accent);
            if labels {
                let d = c.dir.vector();
                let off = Vec2::new(d[0] as f32, d[1] as f32) * 7.0;
                painter.text(
                    at + off,
                    Align2::CENTER_CENTER,
                    &c.id,
                    FontId::monospace(9.5),
                    p.text_secondary,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Categories

/// The first two entries of the category column are not library folders.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum Category {
    /// Favourites and recent symbols; with text typed, every match.
    #[default]
    Start,
    /// The ready-made circuits (macros).
    Circuits,
    /// One chapter of the circuit library (the macro folder `Circuits/3 Hoists`).
    Chapter(String),
    /// One folder of the symbol library (`contacts`, `protection`).
    Library(String),
}

/// One line of the category column.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CategoryLine {
    pub category: Category,
    pub name: String,
    pub icon: Glyph,
    pub count: Option<usize>,
}

/// The name of a library folder as the column writes it.
pub(crate) fn category_name(id: &str) -> String {
    match id {
        "plc" => "PLC".to_owned(),
        "" => String::new(),
        _ => {
            let mut chars = id.replace(['_', '-'], " ").chars().collect::<Vec<_>>();
            chars[0] = chars[0].to_ascii_uppercase();
            chars.into_iter().collect()
        }
    }
}

/// The picture of a library folder: one of the drawn icons, else the place
/// icon.
pub(crate) fn category_icon(id: &str) -> Glyph {
    let name = match id {
        "coils" => "coil",
        "connectivity" => "bridge",
        "contactors" => "contactor",
        "contacts" => "contact-no",
        "controllers" | "plc" => "plc",
        "drives" => "drive",
        "earth" => "ground",
        "measuring" | "sensors" => "sensor",
        "motors" => "motor",
        "network" => "net",
        "operators" => "pushbutton",
        "passive" => "fuse",
        "protection" | "switches" => "breaker",
        "safety" => "estop",
        "signalling" => "lamp",
        "sources" => "potential",
        "supplies" => "psu",
        "terminals" => "terminal",
        "transformers" => "transformer",
        _ => "place",
    };
    Icon::by_name(name)
        .or_else(|| Icon::by_name("place"))
        .map_or(
            Glyph::Phosphor(novo_theme::icons::DOT_OUTLINE),
            Glyph::Drawn,
        )
}

/// The chapters of the circuit library as the category column lists them:
/// the macro folder and how many circuits it holds, in folder order.
pub(crate) fn chapters(macros: &[novo_workflow::palette::MacroEntry]) -> Vec<(String, usize)> {
    let mut out: std::collections::BTreeMap<&str, usize> = Default::default();
    for m in macros.iter().filter(|m| m.folder.starts_with("Circuits/")) {
        *out.entry(m.folder.as_str()).or_default() += 1;
    }
    out.into_iter().map(|(f, n)| (f.to_owned(), n)).collect()
}

/// The category column for this library: Start, Circuits with one line per
/// chapter, then each folder of the library with its symbol count.
pub(crate) fn category_lines(
    library: &Library,
    circuits: usize,
    chapters: &[(String, usize)],
) -> Vec<CategoryLine> {
    let mut folders: std::collections::BTreeMap<&str, usize> = Default::default();
    for symbol in library.iter() {
        *folders.entry(symbol.category.as_str()).or_default() += 1;
    }
    let mut lines = vec![
        CategoryLine {
            category: Category::Start,
            name: "Favourites".to_owned(),
            icon: Glyph::Phosphor(novo_theme::icons::STAR),
            count: None,
        },
        CategoryLine {
            category: Category::Circuits,
            name: "Circuits".to_owned(),
            icon: Icon::by_name("macro")
                .map_or(Glyph::Phosphor(novo_theme::icons::STACK), Glyph::Drawn),
            count: Some(circuits),
        },
    ];
    lines.extend(chapters.iter().map(|(folder, n)| {
        CategoryLine {
            category: Category::Chapter(folder.clone()),
            name: folder
                .strip_prefix("Circuits/")
                .unwrap_or(folder)
                .to_owned(),
            icon: Glyph::Phosphor(novo_theme::icons::DOT_OUTLINE),
            count: Some(*n),
        }
    }));
    lines.extend(folders.into_iter().map(|(id, n)| CategoryLine {
        category: Category::Library(id.to_owned()),
        name: category_name(id),
        icon: category_icon(id),
        count: Some(n),
    }));
    lines
}

// ---------------------------------------------------------------------------
// Which rows a category shows

fn symbol_row(symbol: &Symbol, disabled: Option<&'static str>) -> Row {
    Row {
        target: Target::Symbol(symbol.id.clone()),
        kind: Kind::Symbol,
        label: symbol.name.clone(),
        detail: symbol.category.clone(),
        group: "",
        completion: None,
        shortcut: None,
        disabled,
        score: 0,
    }
}

/// The rows the place palette shows for `category`. `base` are the rows the
/// search gave for the typed text (`query_empty` when nothing is typed).
/// Start shows them as they are; a folder shows its symbols, or the matches
/// that are in it; Circuits shows the macros.
pub(crate) fn rows_for(
    category: &Category,
    base: Vec<Row>,
    query: &str,
    search: &novo_workflow::insert::InsertSearch,
    sources: &Sources<'_>,
) -> Vec<Row> {
    let disabled = (!sources.context.has_page).then_some("No page is open");
    match category {
        Category::Start => base,
        Category::Circuits | Category::Chapter(_) => {
            let query = query.trim().to_lowercase();
            let chapter = match category {
                Category::Chapter(folder) => Some(folder.as_str()),
                _ => None,
            };
            sources
                .macros
                .iter()
                .filter(|m| chapter.is_none_or(|c| m.folder == c))
                .filter(|m| {
                    query.is_empty()
                        || m.name.to_lowercase().contains(&query)
                        || m.words.contains(&query)
                })
                .map(|m| Row {
                    target: Target::Macro(m.name.clone()),
                    kind: Kind::Macro,
                    label: m.name.clone(),
                    detail: m.detail.clone(),
                    group: "",
                    completion: None,
                    shortcut: None,
                    disabled,
                    score: 0,
                })
                .collect()
        }
        Category::Library(folder) => {
            let Some(library) = sources.library else {
                return Vec::new();
            };
            if query.trim().is_empty() {
                let mut symbols: Vec<&Symbol> =
                    library.iter().filter(|s| &s.category == folder).collect();
                symbols.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
                symbols
                    .into_iter()
                    .map(|s| symbol_row(s, disabled))
                    .collect()
            } else {
                search
                    .search(library, query)
                    .into_iter()
                    .filter(|hit| &hit.symbol.category == folder)
                    .map(|hit| symbol_row(hit.symbol, disabled))
                    .collect()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Catalog parts for the detail pane

/// Catalog parts that suit a placed symbol: the ones for its letter code
/// whose function names the symbol's function. A short list on purpose; the
/// Parts window is for browsing.
pub(crate) fn parts_for<'c>(
    catalog: &'c Catalog,
    symbol: &Symbol,
    limit: usize,
) -> Vec<&'c novo_parts::CatalogPart> {
    let Some(letter) = novo_symbols::letter_code(&symbol.function) else {
        return Vec::new();
    };
    let words: Vec<&str> = symbol.function.split('.').collect();
    catalog
        .suggest(letter, "", 200)
        .into_iter()
        .filter(|p| {
            let f = p.function.to_lowercase();
            !f.is_empty()
                && (words
                    .iter()
                    .any(|w| f.contains(w) || w.contains(f.as_str()))
                    || symbol.id.contains(f.as_str())
                    || symbol.keywords.iter().any(|k| k.to_lowercase() == f))
        })
        .take(limit)
        .collect()
}

// ---------------------------------------------------------------------------
// The symbol that stands for a part

/// The library symbol that pictures a catalog part. Only words that name a
/// symbol count, and only when they name exactly one:
/// - the part's function (`pilot lamp`) matches the symbols whose name,
///   function, keywords or aliases contain all its words;
/// - a keyword (`contactor`) must be a symbol's exact alias or name, or one
///   whole segment of its function (`contact.main`), never a loose match.
///
/// One candidate is the picture; several give the one whose name is the
/// term itself; anything still ambiguous (`plc io module` fits inputs and
/// outputs alike) gets no picture, so a part is never shown with a symbol
/// that may be the wrong one.
pub(crate) fn symbol_for_part<'l>(
    library: &'l Library,
    part: &novo_parts::CatalogPart,
) -> Option<&'l Symbol> {
    let exact = |s: &Symbol, term: &str| {
        s.name.to_lowercase() == term
            || s.aliases.iter().any(|a| a == term)
            || s.function.split('.').any(|seg| seg == term)
    };
    let pick = |candidates: Vec<&'l Symbol>, term: &str| match candidates.as_slice() {
        [one] => Some(*one),
        many => {
            let mut named = many.iter().filter(|s| s.name.to_lowercase() == term);
            match (named.next(), named.next()) {
                (Some(one), None) => Some(*one),
                _ => None,
            }
        }
    };
    let function = part.function.trim().to_lowercase();
    let words: Vec<&str> = function
        .split_whitespace()
        .filter(|w| w.chars().count() >= 3)
        .collect();
    if !words.is_empty() {
        let candidates = library
            .iter()
            .filter(|s| {
                let text = format!(
                    "{} {} {} {}",
                    s.name,
                    s.function,
                    s.keywords.join(" "),
                    s.aliases.join(" ")
                )
                .to_lowercase();
                words.iter().all(|w| text.contains(w))
            })
            .collect();
        if let Some(found) = pick(candidates, &function) {
            return Some(found);
        }
    }
    for keyword in &part.keywords {
        let term = keyword.trim().to_lowercase();
        // Colours and variants say how a part looks, not what it is: a
        // green pushbutton is not a green lamp.
        const NOT_A_KIND: [&str; 12] = [
            "green", "red", "yellow", "blue", "white", "black", "amber", "orange", "no", "nc",
            "led", "plc",
        ];
        if term.chars().count() < 3 || NOT_A_KIND.contains(&term.as_str()) {
            continue;
        }
        let candidates = library.iter().filter(|s| exact(s, &term)).collect();
        if let Some(found) = pick(candidates, &term) {
            return Some(found);
        }
    }
    None
}

/// A thumbnail of a symbol in a small square: the drawing in a quiet frame.
pub(crate) fn symbol_thumbnail(ui: &mut egui::Ui, symbol: &Symbol, size: Vec2) -> egui::Response {
    let p = ui_palette();
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    ui.painter().rect_filled(rect, 4.0, p.sunken);
    ui.painter().rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, p.border_subtle),
        egui::StrokeKind::Inside,
    );
    paint_symbol(
        ui.painter(),
        rect.shrink(2.0),
        symbol,
        None,
        p.text_primary,
        false,
        false,
    );
    response
}

// ---------------------------------------------------------------------------
// Widgets

pub(crate) const CARD: Vec2 = Vec2::new(98.0, 84.0);
const CARD_ART: Vec2 = Vec2::new(90.0, 54.0);

/// The category column. Returns the category clicked.
pub(crate) fn category_column(
    ui: &mut egui::Ui,
    lines: &[CategoryLine],
    current: &Category,
) -> Option<Category> {
    let p = ui_palette();
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_salt("place_categories")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for line in lines {
                let selected = &line.category == current;
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 26.0),
                    egui::Sense::click(),
                );
                // The text is painted, so the accessibility tree (screen
                // readers, the QA harness) needs the name from here.
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        selected,
                        line.name.as_str(),
                    )
                });
                let fill = if selected {
                    p.accent_wash
                } else if response.hovered() {
                    p.hover
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter().rect_filled(rect, 4.0, fill);
                let ink = if selected {
                    p.accent_hover
                } else {
                    p.text_primary
                };
                line.icon.paint(
                    ui.painter(),
                    Rect::from_center_size(
                        Pos2::new(rect.left() + 15.0, rect.center().y),
                        Vec2::splat(16.0),
                    ),
                    ink,
                );
                ui.painter().text(
                    Pos2::new(rect.left() + 31.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &line.name,
                    FontId::proportional(13.0),
                    ink,
                );
                if let Some(n) = line.count {
                    ui.painter().text(
                        Pos2::new(rect.right() - 8.0, rect.center().y),
                        Align2::RIGHT_CENTER,
                        n.to_string(),
                        FontId::proportional(11.0),
                        p.text_tertiary,
                    );
                }
                if response.clicked() {
                    clicked = Some(line.category.clone());
                }
            }
        });
    clicked
}

/// What the card grid reports.
#[derive(Debug, Default)]
pub(crate) struct GridOutcome {
    pub clicked: Option<usize>,
    pub hovered: Option<usize>,
}

/// How many cards fit in a row of this width.
pub(crate) fn columns(width: f32) -> usize {
    ((width / (CARD.x + 6.0)).floor() as usize).max(1)
}

/// Height of a group heading line.
const HEADING: f32 = 18.0;
const GAP: f32 = 6.0;

/// Where each card and group heading sits in the grid, relative to its
/// top left, and the height of all of it. Pure, so the scrolling grid only
/// paints what its viewport shows.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct GridLayout {
    pub cards: Vec<Pos2>,
    pub headings: Vec<(f32, &'static str)>,
    pub height: f32,
}

pub(crate) fn grid_layout(rows: &[Row], per_row: usize) -> GridLayout {
    let mut out = GridLayout::default();
    let mut y = 0.0;
    let mut group = "";
    let mut in_line = 0;
    for row in rows {
        let new_group = !row.group.is_empty() && row.group != group;
        if new_group || in_line == per_row {
            if in_line > 0 {
                y += CARD.y + GAP;
            }
            in_line = 0;
        }
        if new_group {
            group = row.group;
            out.headings.push((y, group));
            y += HEADING;
        }
        out.cards
            .push(Pos2::new(in_line as f32 * (CARD.x + GAP), y));
        in_line += 1;
    }
    out.height = if in_line > 0 { y + CARD.y } else { y };
    out
}

/// The scale the cards are drawn at, worked out once per list of rows and
/// kept in egui's memory.
fn grid_scale(ui: &egui::Ui, rows: &[Row], symbols: &[Option<&Symbol>]) -> f32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for row in rows {
        row.target.hash(&mut hasher);
    }
    let key = hasher.finish();
    let id = egui::Id::new("place_grid_scale");
    if let Some((k, scale)) = ui.data(|d| d.get_temp::<(u64, f32)>(id))
        && k == key
    {
        return scale;
    }
    let scale = common_scale(symbols.iter().flatten().copied(), CARD_ART, 4.0);
    ui.data_mut(|d| d.insert_temp(id, (key, scale)));
    scale
}

/// The card grid: each row as a card with the real symbol drawing at one
/// scale. Only the cards in view are painted, so a folder of hundreds of
/// symbols costs the same as one of twenty. `scroll_to` scrolls the
/// selected card into view.
pub(crate) fn card_grid(
    ui: &mut egui::Ui,
    rows: &[Row],
    library: Option<&Library>,
    selected: usize,
    scroll_to: bool,
) -> GridOutcome {
    let p = ui_palette();
    let mut out = GridOutcome::default();
    let symbols: Vec<Option<&Symbol>> = rows
        .iter()
        .map(|r| match &r.target {
            Target::Symbol(id) => library.and_then(|l| l.get(id)),
            _ => None,
        })
        .collect();
    let scale = grid_scale(ui, rows, &symbols);
    let layout = grid_layout(rows, columns(ui.available_width()));
    egui::ScrollArea::vertical()
        .id_salt("place_grid")
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            let origin = ui.cursor().min;
            ui.set_height(layout.height);
            for (y, name) in &layout.headings {
                if *y + HEADING < viewport.top() || *y > viewport.bottom() {
                    continue;
                }
                ui.painter().text(
                    origin + Vec2::new(2.0, *y + 2.0),
                    Align2::LEFT_TOP,
                    name.to_uppercase(),
                    FontId::proportional(10.5),
                    p.text_secondary,
                );
            }
            for (j, row) in rows.iter().enumerate() {
                let rect = Rect::from_min_size(origin + layout.cards[j].to_vec2(), CARD);
                if j == selected && scroll_to {
                    ui.scroll_to_rect(rect, None);
                }
                let top = layout.cards[j].y;
                if top + CARD.y < viewport.top() || top > viewport.bottom() {
                    continue;
                }
                let response = ui.interact(rect, ui.id().with(("card", j)), egui::Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        row.disabled.is_none(),
                        j == selected,
                        row.label.as_str(),
                    )
                });
                let enabled = row.disabled.is_none();
                let fill = if j == selected {
                    p.accent_wash
                } else if response.hovered() && enabled {
                    p.hover
                } else {
                    p.control
                };
                let border = if j == selected {
                    p.accent
                } else {
                    p.border_subtle
                };
                ui.painter().rect_filled(rect, 6.0, fill);
                ui.painter().rect_stroke(
                    rect,
                    6.0,
                    Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
                let ink = if enabled {
                    p.text_primary
                } else {
                    p.text_tertiary
                };
                let art = Rect::from_min_size(rect.min + Vec2::new(4.0, 4.0), CARD_ART);
                match symbols[j] {
                    Some(symbol) => {
                        paint_symbol(ui.painter(), art, symbol, Some(scale), ink, false, false);
                    }
                    None => {
                        let glyph = match row.kind {
                            Kind::Macro => Icon::by_name("macro")
                                .map_or(Glyph::Phosphor(novo_theme::icons::STACK), Glyph::Drawn),
                            _ => Glyph::Phosphor(novo_theme::icons::DOT_OUTLINE),
                        };
                        glyph.paint(
                            ui.painter(),
                            Rect::from_center_size(art.center(), Vec2::splat(30.0)),
                            ink,
                        );
                    }
                }
                novo_theme::widgets::paint_text_ellipsis(
                    ui.painter(),
                    Pos2::new(rect.left() + 6.0, rect.bottom() - 11.0),
                    &row.label,
                    FontId::proportional(11.5),
                    ink,
                    CARD.x - 12.0,
                );
                if let Some(key) = &row.shortcut {
                    ui.painter().text(
                        rect.min + Vec2::new(7.0, 8.0),
                        Align2::LEFT_TOP,
                        key,
                        FontId::monospace(10.0),
                        p.accent_hover,
                    );
                }
                if response.hovered() {
                    out.hovered = Some(j);
                }
                if response.clicked() && enabled {
                    out.clicked = Some(j);
                }
                // Labels are cut to the card, so the full name is the tooltip.
                match row.disabled {
                    Some(reason) => response.on_hover_text(format!("{}\n{reason}", row.label)),
                    None => response.on_hover_text(&row.label),
                };
            }
        });
    out
}

/// The right pane: the selected symbol large with its pins, the tag prefix
/// and the catalog parts that suit it; for a circuit its description.
pub(crate) fn detail_pane(
    ui: &mut egui::Ui,
    row: Option<&Row>,
    library: Option<&Library>,
    catalog: Option<&Catalog>,
) {
    let p = ui_palette();
    let Some(row) = row else {
        ui.label(egui::RichText::new("Nothing selected").color(p.text_tertiary));
        return;
    };
    let symbol = match &row.target {
        Target::Symbol(id) => library.and_then(|l| l.get(id)),
        _ => None,
    };
    let width = ui.available_width();
    let (art, _) = ui.allocate_exact_size(Vec2::new(width, 140.0), egui::Sense::hover());
    ui.painter().rect_filled(art, 6.0, p.sunken);
    ui.painter().rect_stroke(
        art,
        6.0,
        Stroke::new(1.0, p.border_subtle),
        egui::StrokeKind::Inside,
    );
    if let Some(symbol) = symbol {
        paint_symbol(
            ui.painter(),
            art.shrink(10.0),
            symbol,
            None,
            p.text_primary,
            true,
            true,
        );
    } else if let Some(icon) = Icon::by_name("macro") {
        novo_theme::drawn::paint(
            ui.painter(),
            Rect::from_center_size(art.center(), Vec2::splat(56.0)),
            icon,
            p.text_secondary,
        );
    }
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(&row.label)
            .strong()
            .color(p.text_strong),
    );
    match symbol {
        Some(symbol) => {
            if !symbol.description.is_empty() {
                ui.label(
                    egui::RichText::new(&symbol.description)
                        .small()
                        .color(p.text_secondary),
                );
            }
            ui.add_space(4.0);
            if let Some(code) = novo_symbols::letter_code(&symbol.function) {
                detail_line(ui, "Tag", &format!("-{code}1"));
            }
            detail_line(ui, "Standard", &symbol.standard);
            detail_line(ui, "Function", &symbol.function);
            if !symbol.connections.is_empty() {
                let pins: Vec<&str> = symbol.connections.iter().map(|c| c.id.as_str()).collect();
                detail_line(ui, "Pins", &pins.join("  "));
            }
            if !symbol.aliases.is_empty() {
                detail_line(ui, "Type", &symbol.aliases.join("  "));
            }
            if let Some(catalog) = catalog {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("PARTS").small().color(p.text_secondary));
                let parts = parts_for(catalog, symbol, 3);
                if parts.is_empty() {
                    ui.label(
                        egui::RichText::new("No catalog part is listed for this symbol")
                            .small()
                            .color(p.text_tertiary),
                    );
                }
                for part in parts {
                    ui.label(
                        egui::RichText::new(format!("{} {}", part.manufacturer, part.order_number))
                            .small()
                            .color(p.text_primary),
                    );
                }
            }
        }
        None => {
            if !row.detail.is_empty() {
                ui.label(
                    egui::RichText::new(&row.detail)
                        .small()
                        .color(p.text_secondary),
                );
            }
        }
    }
}

fn detail_line(ui: &mut egui::Ui, name: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    let p = ui_palette();
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(name).small().color(p.text_secondary));
        ui.label(egui::RichText::new(value).small().color(p.text_primary));
    });
}

// ---------------------------------------------------------------------------
// Command palette row pieces

/// The picture in front of a command palette row.
pub(crate) fn kind_glyph(row: &Row) -> Glyph {
    use novo_theme::icons as i;
    match &row.target {
        Target::Command(id) | Target::Run { command: id, .. } => {
            novo_theme::presentation::glyph(id)
        }
        Target::Addon(id) => novo_theme::presentation::glyph(id),
        _ => Glyph::Phosphor(match row.kind {
            Kind::Page => i::FILE_TEXT,
            Kind::Device | Kind::Location => i::PACKAGE,
            Kind::Cable => i::PLUGS,
            Kind::Wire => i::ARROW_UP_RIGHT,
            Kind::Symbol => i::DOT_OUTLINE,
            Kind::Lesson => i::GRADUATION_CAP,
            Kind::Macro => i::STACK,
            Kind::Command => i::DOT_OUTLINE,
        }),
    }
}

/// The type chip of a row: what it is, in one word; an add-on's command
/// names the add-on (`Add-on: Controllers`).
pub(crate) fn type_chip(row: &Row) -> String {
    match &row.target {
        Target::Addon(_) => match addon_parts(row) {
            Some((name, _)) => format!("Add-on: {}", shorten(name, 22)),
            None => "Add-on".to_owned(),
        },
        _ => row.kind.label().to_owned(),
    }
}

/// The add-on's name and the rest of the row's detail (its description),
/// from the detail `Add-on: name` or `Add-on: name · description`.
pub(crate) fn addon_parts(row: &Row) -> Option<(&str, &str)> {
    let rest = row.detail.strip_prefix("Add-on: ")?;
    Some(match rest.split_once(" \u{b7} ") {
        Some((name, description)) => (name, description),
        None => (rest, ""),
    })
}

fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(max.saturating_sub(1)).collect();
    cut.push('\u{2026}');
    cut
}

/// A small outlined label: the type or the menu path of a row.
pub(crate) fn paint_chip(
    painter: &Painter,
    right: f32,
    centre_y: f32,
    text: &str,
    accent: bool,
) -> f32 {
    let p = ui_palette();
    let font = FontId::proportional(10.5);
    let galley = painter.layout_no_wrap(text.to_owned(), font, p.text_secondary);
    let size = galley.size() + Vec2::new(10.0, 4.0);
    let rect = Rect::from_min_size(Pos2::new(right - size.x, centre_y - size.y / 2.0), size);
    let (fill, line, ink) = if accent {
        (p.accent_wash, p.accent, p.accent_hover)
    } else {
        (p.control, p.border_subtle, p.text_secondary)
    };
    painter.rect_filled(rect, size.y / 2.0, fill);
    painter.rect_stroke(
        rect,
        size.y / 2.0,
        Stroke::new(1.0, line),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.min + Vec2::new(5.0, 2.0), galley, ink);
    rect.left() - 6.0
}

/// The key cap of a shortcut, painted right-aligned ending at `right`.
pub(crate) fn paint_key_cap(painter: &Painter, right: f32, centre_y: f32, text: &str) -> f32 {
    let p = ui_palette();
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::monospace(10.5), p.text_secondary);
    let size = galley.size() + Vec2::new(10.0, 4.0);
    let rect = Rect::from_min_size(Pos2::new(right - size.x, centre_y - size.y / 2.0), size);
    painter.rect_filled(rect, 4.0, p.control);
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, p.border_subtle),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.min + Vec2::new(5.0, 2.0), galley, p.text_secondary);
    rect.left() - 6.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Library {
        Library::builtin().unwrap()
    }

    #[test]
    fn every_symbol_has_artwork_inside_a_finite_box() {
        let lib = library();
        for symbol in lib.iter() {
            assert!(!pieces(symbol).is_empty(), "{}", symbol.id);
            let (w, h) = extent(symbol);
            assert!(
                w.is_finite() && h.is_finite() && w >= 4.0 && h >= 4.0,
                "{}",
                symbol.id
            );
        }
    }

    #[test]
    fn each_circuit_chapter_is_a_line_that_shows_only_its_circuits() {
        let entry = |name: &str, folder: &str| novo_workflow::palette::MacroEntry {
            name: name.into(),
            folder: folder.into(),
            detail: String::new(),
            words: String::new(),
        };
        let macros = vec![
            entry("Starter one", "Starter"),
            entry("Hoist a", "Circuits/3 Hoists"),
            entry("Hoist b", "Circuits/3 Hoists"),
            entry("Horn", "Circuits/7 Sensors and signalling"),
        ];
        let chapters = chapters(&macros);
        assert_eq!(
            chapters,
            vec![
                ("Circuits/3 Hoists".to_owned(), 2),
                ("Circuits/7 Sensors and signalling".to_owned(), 1)
            ]
        );
        let lines = category_lines(&library(), 4, &chapters);
        assert_eq!(lines[2].name, "3 Hoists");
        assert_eq!(
            lines[2].category,
            Category::Chapter("Circuits/3 Hoists".into())
        );
    }

    #[test]
    fn every_folder_has_a_line_and_a_picture() {
        let lib = library();
        let lines = category_lines(&lib, 3, &[]);
        assert_eq!(lines[0].category, Category::Start);
        assert_eq!(lines[1].category, Category::Circuits);
        let folders = lines.len() - 2;
        assert!(folders >= 21, "{folders} folders");
        let total: usize = lines.iter().skip(2).filter_map(|l| l.count).sum();
        assert_eq!(total, lib.len());
        for line in &lines {
            assert!(!line.name.is_empty());
            if let Category::Library(id) = &line.category {
                assert!(
                    category_icon(id).is_drawn(),
                    "folder {id} has no drawn icon"
                );
            }
        }
    }

    #[test]
    fn the_common_scale_keeps_relative_size() {
        let lib = library();
        let cell = CARD_ART;
        let scale = common_scale(lib.iter(), cell, 4.0);
        assert!(scale > 0.0 && scale <= MAX_SCALE);
        // At least four fifths of the library fits at the common scale.
        let fitting = lib
            .iter()
            .filter(|s| fit_scale(s, cell, 4.0) >= scale - 1e-4)
            .count();
        assert!(fitting * 5 >= lib.len() * 4, "{fitting} of {}", lib.len());
        // A contact is smaller on screen than a motor, as it is on paper.
        let no = lib.get("iec/contacts/no").unwrap();
        let motor = lib.get("iec/motors/motor_3ph").unwrap();
        let drawn = |s: &Symbol| {
            let (w, h) = extent(s);
            w.max(h) * scale.min(fit_scale(s, cell, 4.0))
        };
        assert!(drawn(no) < drawn(motor));
    }

    #[test]
    fn category_names_read_well() {
        assert_eq!(category_name("plc"), "PLC");
        assert_eq!(category_name("contacts"), "Contacts");
        assert_eq!(category_name("my_things"), "My things");
    }

    #[test]
    fn a_folder_lists_its_symbols_and_filters_matches() {
        let lib = library();
        let keymap = novo_workflow::keymap::Keymap::default();
        let sources = Sources {
            project: None,
            library: Some(&lib),
            keymap: &keymap,
            context: novo_workflow::commands::Context::default(),
            lessons: &[],
            macros: &[],
            addons: &[],
        };
        let search = novo_workflow::insert::InsertSearch::default();
        let folder = Category::Library("contacts".to_owned());
        let rows = rows_for(&folder, Vec::new(), "", &search, &sources);
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|r| match &r.target {
            Target::Symbol(id) => lib.get(id).unwrap().category == "contacts",
            _ => false,
        }));
        let typed = rows_for(&folder, Vec::new(), "contact", &search, &sources);
        assert!(!typed.is_empty() && typed.len() <= rows.len());
    }

    #[test]
    fn symbols_paint_headlessly_with_pins() {
        let lib = library();
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(200.0, 140.0), egui::Sense::hover());
            for symbol in lib.iter() {
                paint_symbol(ui.painter(), rect, symbol, None, Color32::WHITE, true, true);
            }
        });
        out.textures_delta.clear();
    }

    #[test]
    fn parts_for_never_panics_on_any_symbol() {
        let lib = library();
        let catalog = Catalog::starter();
        for symbol in lib.iter() {
            let _ = parts_for(&catalog, symbol, 3);
        }
    }
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    fn row(label: &str, group: &'static str) -> Row {
        Row {
            target: Target::Symbol(label.to_owned()),
            kind: Kind::Symbol,
            label: label.to_owned(),
            detail: String::new(),
            group,
            completion: None,
            shortcut: None,
            disabled: None,
            score: 0,
        }
    }

    #[test]
    fn cards_wrap_and_headings_start_a_new_line() {
        let rows = vec![
            row("a", "Favourites"),
            row("b", "Favourites"),
            row("c", "Favourites"),
            row("d", "Recent"),
        ];
        let l = grid_layout(&rows, 2);
        assert_eq!(l.headings.len(), 2);
        assert_eq!(l.cards[0], Pos2::new(0.0, HEADING));
        assert_eq!(l.cards[1].x, CARD.x + GAP);
        assert_eq!(l.cards[2].x, 0.0);
        assert!(l.cards[2].y > l.cards[0].y);
        // The next group starts below the last card of the one before.
        assert!(l.headings[1].0 >= l.cards[2].y + CARD.y);
        assert_eq!(l.cards[3].y, l.headings[1].0 + HEADING);
        assert_eq!(l.height, l.cards[3].y + CARD.y);
    }

    /// A folder of a thousand cards paints only the ones in view.
    #[test]
    fn a_long_list_paints_only_the_visible_cards() {
        let lib = Library::builtin().unwrap();
        let one = lib.get("iec/contacts/no").unwrap();
        let rows: Vec<Row> = (0..1000).map(|_| symbol_row(one, None)).collect();
        let ctx = egui::Context::default();
        let mut shapes = 0;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
                ..Default::default()
            },
            |ui| {
                card_grid(ui, &rows, Some(&lib), 0, false);
            },
        );
        out.textures_delta.clear();
        shapes += out.shapes.len();
        // About 4 lines of 4 cards fit; each card is a few shapes plus art.
        assert!(shapes < 400, "{shapes} shapes for 1000 rows");
    }
}

#[cfg(test)]
mod part_symbol_tests {
    use super::*;

    #[test]
    fn a_part_is_pictured_by_a_symbol_that_its_words_name() {
        let lib = Library::builtin().unwrap();
        let catalog = Catalog::starter();
        let mut pictured = 0;
        for part in catalog.parts() {
            let Some(symbol) = symbol_for_part(&lib, part) else {
                continue;
            };
            pictured += 1;
            // The picture is never a guess: a word of the part is in the symbol.
            let text = format!(
                "{} {} {} {}",
                symbol.name,
                symbol.function,
                symbol.keywords.join(" "),
                symbol.aliases.join(" ")
            )
            .to_lowercase();
            let words = part
                .function
                .split_whitespace()
                .chain(part.keywords.iter().map(String::as_str));
            assert!(
                words
                    .map(str::to_lowercase)
                    .any(|w| w.chars().count() >= 3 && text.contains(&w)),
                "{} {} -> {}",
                part.manufacturer,
                part.order_number,
                symbol.id
            );
        }
        assert!(
            pictured >= 10,
            "only {pictured} of {} parts have a picture",
            catalog.len()
        );
    }

    #[test]
    fn the_add_on_chip_names_the_add_on_and_the_detail_keeps_the_description() {
        let row = |detail: &str| Row {
            target: Target::Addon("addon.ctl.run".into()),
            kind: Kind::Command,
            label: "Run".into(),
            detail: detail.into(),
            group: "",
            completion: None,
            shortcut: None,
            disabled: None,
            score: 0,
        };
        assert_eq!(
            type_chip(&row("Add-on: Controllers \u{b7} Starts it")),
            "Add-on: Controllers"
        );
        assert_eq!(
            addon_parts(&row("Add-on: Controllers \u{b7} Starts it")),
            Some(("Controllers", "Starts it"))
        );
        assert_eq!(
            addon_parts(&row("Add-on: Controllers")),
            Some(("Controllers", ""))
        );
        assert_eq!(type_chip(&row("")), "Add-on");
    }
}
