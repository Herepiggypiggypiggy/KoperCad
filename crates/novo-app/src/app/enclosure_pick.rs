//! Choosing a real enclosure when a cabinet is made.
//!
//! The "New cabinet" window lists enclosures as cards: the enclosures the
//! user imported in Parts (a maker's STEP model read at import), the user's
//! own enclosure list, and shipped catalogue rows (none ship until the
//! maker's terms are checked, `handoff/cabinet-start/DESIGN.md`). Choosing a
//! card gives the cabinet an [`EnclosureSpec`] with the source of every
//! number, saved with the cabinet, so heat, door and reports read the same
//! enclosure (`handoff/cabinet-start/CHAIN.md`).
//!
//! Reach it: Outline, New cabinet, "Enclosure".

use eframe::egui::{self, Sense, Stroke, Vec2};
use novo_core::{MountingPlate, Value};
use novo_layout::sizing::EnclosureSize;
use novo_layout::spec::{CatalogueRow, EnclosureSpec, Field, Source};
use novo_parts::Catalog;
use novo_parts::body::keys;

use crate::theme::ui_palette;

/// Plate border when the enclosure does not say.
const DEFAULT_MARGIN_MM: f64 = 10.0;
/// Plate smaller than the inside by this much per side when the plate size
/// is not known (mounting plates stand off the walls); shown as a default.
const PLATE_GAP_W_MM: f64 = 100.0;
const PLATE_GAP_H_MM: f64 = 200.0;

/// One enclosure the user can pick.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Choice {
    /// Heading of the card.
    pub title: String,
    /// Where the card comes from, in words.
    pub origin: &'static str,
    pub spec: EnclosureSpec,
    pub plate: MountingPlate,
    /// The plate size is a default, not the maker's.
    pub plate_is_default: bool,
    /// The name in the user's enclosure list, kept as the old property.
    pub list_name: Option<String>,
}

impl Choice {
    pub fn size_text(&self) -> String {
        let d = |f: &Option<Field>| f.map_or("?".to_owned(), |f| format!("{}", f.value));
        format!(
            "{} x {} x {} mm",
            d(&self.spec.width),
            d(&self.spec.height),
            d(&self.spec.depth)
        )
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.is_empty()
            || self.title.to_lowercase().contains(&q)
            || self.spec.article.to_lowercase().contains(&q)
            || self.size_text().contains(&q)
    }
}

fn number(part: &novo_parts::CatalogPart, key: &str) -> Option<f64> {
    match part.data.get(key) {
        Some(Value::Float(v)) => Some(*v),
        Some(Value::Int(v)) => Some(*v as f64),
        _ => None,
    }
}

/// Enclosures among the user's imported parts.
pub(super) fn from_parts(catalog: &Catalog) -> Vec<Choice> {
    let mut out: Vec<Choice> = catalog
        .parts()
        .iter()
        .filter(|p| matches!(p.data.get(keys::BODY_KIND), Some(Value::Text(t)) if t == "enclosure"))
        .filter_map(|p| {
            let (w, h, d) = (
                number(p, keys::OUTER_WIDTH_MM)?,
                number(p, keys::OUTER_HEIGHT_MM)?,
                number(p, keys::OUTER_DEPTH_MM)?,
            );
            let c = |v: f64| Some(Field::new(v, Source::Catalogue));
            Some(Choice {
                title: format!("{} {}", p.manufacturer, p.order_number),
                origin: "Your parts",
                spec: EnclosureSpec {
                    article: p.order_number.clone(),
                    series: p.type_number.clone(),
                    width: c(w),
                    height: c(h),
                    depth: c(d),
                    ..EnclosureSpec::default()
                },
                plate: MountingPlate {
                    width: (w - PLATE_GAP_W_MM).max(0.0),
                    height: (h - PLATE_GAP_H_MM).max(0.0),
                    margin: DEFAULT_MARGIN_MM,
                },
                plate_is_default: true,
                list_name: None,
            })
        })
        .collect();
    out.sort_by(|a, b| a.title.cmp(&b.title));
    out
}

/// The user's own list of sizes (typed by the user).
pub(super) fn from_list(items: &[EnclosureSize]) -> Vec<Choice> {
    items
        .iter()
        .map(|e| {
            let t = |v: f64| Some(Field::new(v, Source::Typed));
            Choice {
                title: e.name.clone(),
                origin: "Your list",
                spec: EnclosureSpec {
                    width: t(e.width),
                    height: t(e.height),
                    depth: t(e.depth),
                    ..EnclosureSpec::default()
                },
                plate: MountingPlate {
                    width: e.plate_width,
                    height: e.plate_height,
                    margin: e.margin,
                },
                plate_is_default: false,
                list_name: Some(e.name.clone()),
            }
        })
        .collect()
}

/// Shipped catalogue rows: none until the maker's terms are checked.
pub(super) fn from_rows(rows: &[CatalogueRow]) -> Vec<Choice> {
    rows.iter()
        .map(|r| Choice {
            title: format!("{} {}", r.series, r.article),
            origin: "Catalogue",
            spec: r.spec(),
            plate: MountingPlate {
                width: (r.width - PLATE_GAP_W_MM).max(0.0),
                height: (r.height - PLATE_GAP_H_MM).max(0.0),
                margin: DEFAULT_MARGIN_MM,
            },
            plate_is_default: true,
            list_name: None,
        })
        .collect()
}

/// Every card, in the order shown.
pub(super) fn all(catalog: Option<&Catalog>, list: &[EnclosureSize]) -> Vec<Choice> {
    let mut out = catalog.map(from_parts).unwrap_or_default();
    out.extend(from_rows(&[]));
    out.extend(from_list(list));
    out
}

/// What the picker keeps between frames.
#[derive(Debug, Default, Clone)]
pub(super) struct Picker {
    pub query: String,
    /// Index into the choices; `None` is the standard plate.
    pub picked: Option<usize>,
}

/// The cards. Returns true when the pick changed.
pub(super) fn show(ui: &mut egui::Ui, picker: &mut Picker, choices: &[Choice]) -> bool {
    let p = ui_palette();
    let before = picker.picked;
    ui.add(
        egui::TextEdit::singleline(&mut picker.query)
            .desired_width(f32::INFINITY)
            .hint_text("Search: 8455, 600 x 1800"),
    );
    ui.add_space(6.0);
    let mut shown = 0;
    egui::ScrollArea::vertical()
        .id_salt("enclosure_pick")
        .max_height(180.0)
        .show(ui, |ui| {
            let width = ui.available_width();
            for (i, c) in choices.iter().enumerate() {
                if !c.matches(&picker.query) {
                    continue;
                }
                shown += 1;
                let selected = picker.picked == Some(i);
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(width, 52.0), Sense::click());
                let fill = if selected {
                    p.hover
                } else if response.hovered() {
                    p.control
                } else {
                    p.sunken
                };
                let stroke = if selected {
                    Stroke::new(1.5, p.border_strong)
                } else {
                    Stroke::new(1.0, p.border_subtle)
                };
                ui.painter()
                    .rect(rect, 6.0, fill, stroke, egui::StrokeKind::Inside);
                let text_at = rect.min + Vec2::new(12.0, 8.0);
                ui.painter().text(
                    text_at,
                    egui::Align2::LEFT_TOP,
                    &c.title,
                    egui::FontId::proportional(14.0),
                    p.text_strong,
                );
                ui.painter().text(
                    text_at + Vec2::new(0.0, 20.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}  ·  {}", c.size_text(), c.origin),
                    egui::FontId::proportional(12.0),
                    p.text_secondary,
                );
                if response.clicked() {
                    picker.picked = if selected { None } else { Some(i) };
                }
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        selected,
                        c.title.clone(),
                    )
                });
            }
        });
    if shown == 0 {
        ui.weak(if choices.is_empty() {
            "No enclosures yet"
        } else {
            "Nothing matches"
        });
    }
    picker.picked != before
}

/// The words next to a value: a catalogue number that came from the user's own
/// parts says so.
fn words(c: &Choice, f: &Field) -> &'static str {
    if f.source == Source::Catalogue && c.origin == "Your parts" {
        "from your parts"
    } else {
        f.source.words()
    }
}

/// One line per value saying where it came from (behind the "?").
pub(super) fn state_lines(c: &Choice) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = |name: &str, f: &Option<Field>| {
        if let Some(f) = f {
            out.push(format!("{name} {} mm, {}", f.value, words(c, f)));
        }
    };
    line("Width", &c.spec.width);
    line("Height", &c.spec.height);
    line("Depth", &c.spec.depth);
    line("Wall", &c.spec.wall);
    out.push(format!(
        "Plate {} x {} mm, {}",
        c.plate.width,
        c.plate.height,
        if c.plate_is_default {
            Source::Default.words()
        } else {
            "your list"
        }
    ));
    out
}

/// True while some number is a catalogue or default value, the plate size
/// included (a plate not read from the maker's own file is a default).
pub(super) fn is_estimate(c: &Choice) -> bool {
    c.spec.notice().is_some() || c.plate_is_default
}

/// The one line shown for a pick: size and plate.
pub(super) fn summary(c: &Choice) -> String {
    format!(
        "{}  ·  plate {} x {} mm",
        c.size_text(),
        c.plate.width,
        c.plate.height
    )
}

/// What the "?" says: the estimate sentence (when it applies) and where each
/// value came from.
pub(super) fn detail(c: &Choice) -> String {
    let mut text = String::new();
    if is_estimate(c) {
        text.push_str(novo_layout::spec::ESTIMATE_NOTICE);
        text.push_str("\n\n");
    }
    text.push_str(&state_lines(c).join("\n"));
    text
}

// ---------------------------------------------------------------------
// The maker's STEP file

/// What the STEP file the person gave says about the enclosure.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct StepRead {
    pub file_name: String,
    pub sha256: String,
    /// Names the file carries (file name, product names): where an article
    /// number would be.
    pub names: Vec<String>,
    /// Width, height, depth of the solid, mm.
    pub size: [f64; 3],
    /// The order of the three sizes came from the picked enclosure (else
    /// it is the usual guess: middle, largest, smallest).
    pub order_known: bool,
    pub notes: Vec<String>,
}

/// Does the file belong to the picked enclosure?
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ArticleCheck {
    Matches,
    /// The file names another article; carries the file's name.
    Differs {
        file: String,
    },
    /// Nothing to compare (no article picked, or the file names nothing).
    Unknown,
}

/// Reads the STEP file (or the STEP inside a ZIP) at `path`. `hint` is the
/// size text of the picked enclosure ("600 x 1800 x 500 mm"), which gives the
/// order of width, height and depth when it matches the solid.
pub(super) fn read_step_file(path: &std::path::Path, hint: &str) -> Result<StepRead, String> {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read the file: {e}"))?;
    read_step_bytes(&file_name, &bytes, hint)
}

fn step_in_zip(bytes: &[u8]) -> Result<(String, Vec<u8>), String> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| "This ZIP could not be opened".to_owned())?;
    for i in 0..zip.len() {
        let Ok(f) = zip.by_index(i) else { continue };
        let Ok(name) = f.name().map(|n| n.into_owned()) else {
            continue;
        };
        let lower = name.to_lowercase();
        if f.is_file() && (lower.ends_with(".stp") || lower.ends_with(".step")) {
            let mut out = Vec::new();
            f.take(300_000_000)
                .read_to_end(&mut out)
                .map_err(|_| "The STEP file in the ZIP could not be read".to_owned())?;
            let base = name.rsplit(['/', '\\']).next().unwrap_or(&name).to_owned();
            return Ok((base, out));
        }
    }
    Err("There is no STEP file in this ZIP".into())
}

pub(super) fn read_step_bytes(
    file_name: &str,
    bytes: &[u8],
    hint: &str,
) -> Result<StepRead, String> {
    use sha2::{Digest, Sha256};
    let lower = file_name.to_lowercase();
    let (name, data) = if lower.ends_with(".zip") {
        step_in_zip(bytes)?
    } else if lower.ends_with(".stp") || lower.ends_with(".step") {
        (file_name.to_owned(), bytes.to_vec())
    } else {
        return Err("This is not a STEP file (.stp or .step) or a ZIP with one".into());
    };
    let sha256 = Sha256::digest(&data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let text = String::from_utf8_lossy(&data);
    let reading = novo_interop::part3d::read_body(&text)?;
    if reading.kind == novo_parts::body::BodyKind::MountingPlate {
        return Err("This is a mounting plate, not an enclosure".into());
    }
    let ([w, h, d], order_known) = reading.outer_size(hint);
    let identity = novo_parts::CadIdentity::read(&name, &data);
    let mut notes = reading.notes.clone();
    if !order_known {
        notes.push(
            "The order of width, height and depth is a guess from the solid: check it.".into(),
        );
    }
    Ok(StepRead {
        file_name: name,
        sha256,
        names: identity.names.iter().map(|n| n.text.clone()).collect(),
        size: [w, h, d],
        order_known,
        notes,
    })
}

fn article_check(picked: &str, names: &[String]) -> ArticleCheck {
    let want = novo_parts::normalize_code(picked);
    if want.is_empty() || names.is_empty() {
        return ArticleCheck::Unknown;
    }
    if names
        .iter()
        .any(|n| novo_parts::normalize_code(n).contains(&want))
    {
        return ArticleCheck::Matches;
    }
    ArticleCheck::Differs {
        file: names
            .iter()
            .find(|n| n.chars().any(|c| c.is_ascii_digit()))
            .unwrap_or(&names[0])
            .clone(),
    }
}

/// The plate of an enclosure whose plate size is not known.
fn default_plate(w: f64, h: f64, margin: f64) -> MountingPlate {
    MountingPlate {
        width: (w - PLATE_GAP_W_MM).max(0.0),
        height: (h - PLATE_GAP_H_MM).max(0.0),
        margin,
    }
}

/// What the picked enclosure becomes once its STEP file is read.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Applied {
    pub choice: Choice,
    pub article: ArticleCheck,
    /// One line per size where the file and the card disagree, both numbers.
    pub conflicts: Vec<String>,
}

/// The STEP size is used over the card's (the file is the maker's own
/// model); a disagreement beyond the tolerance is listed with both numbers.
pub(super) fn apply_step(c: &Choice, s: &StepRead) -> Applied {
    use novo_layout::spec::reconcile;
    let mut out = c.clone();
    let mut conflicts = Vec::new();
    let mut one = |name: &str, step: f64, card: &Option<Field>, slot: &mut Option<Field>| {
        if let Some(r) = reconcile(Some(step), card.map(|f| f.value)) {
            if let Some((file, card)) = r.conflict {
                conflicts.push(format!(
                    "{name}: file {file} mm, {} {card} mm",
                    c.origin_words()
                ));
            }
            *slot = Some(r.field);
        }
    };
    one("Width", s.size[0], &c.spec.width, &mut out.spec.width);
    one("Height", s.size[1], &c.spec.height, &mut out.spec.height);
    one("Depth", s.size[2], &c.spec.depth, &mut out.spec.depth);
    out.spec.step_name = s.file_name.clone();
    out.spec.step_sha256 = s.sha256.clone();
    if out.plate_is_default {
        out.plate = default_plate(s.size[0], s.size[1], c.plate.margin);
    }
    Applied {
        article: article_check(&c.spec.article, &s.names),
        choice: out,
        conflicts,
    }
}

/// An enclosure made from the STEP file alone (no card picked).
pub(super) fn choice_from_step(s: &StepRead) -> Choice {
    let f = |v: f64| Some(Field::new(v, Source::Step));
    Choice {
        title: s.file_name.clone(),
        origin: "Your STEP file",
        spec: EnclosureSpec {
            width: f(s.size[0]),
            height: f(s.size[1]),
            depth: f(s.size[2]),
            step_name: s.file_name.clone(),
            step_sha256: s.sha256.clone(),
            ..EnclosureSpec::default()
        },
        plate: default_plate(s.size[0], s.size[1], DEFAULT_MARGIN_MM),
        plate_is_default: true,
        list_name: None,
    }
}

impl Choice {
    fn origin_words(&self) -> &'static str {
        match self.origin {
            "Your parts" => "your parts",
            "Your list" => "your list",
            _ => "catalogue",
        }
    }
}

type Pending = std::sync::Arc<std::sync::Mutex<Option<Result<StepRead, String>>>>;

/// The STEP file of the form: being read on a worker thread (a maker's model
/// can be many megabytes), read, or refused.
#[derive(Debug, Clone, Default)]
pub(super) struct StepSlot {
    pending: Option<Pending>,
    pub read: Option<StepRead>,
    pub error: Option<String>,
    /// Where the drop zone was last frame, for files dropped on it.
    pub drop_rect: Option<egui::Rect>,
}

impl StepSlot {
    pub fn is_reading(&self) -> bool {
        self.pending.is_some()
    }

    /// Starts reading `path`; the result appears in a later frame.
    pub fn start(&mut self, ctx: &egui::Context, path: std::path::PathBuf, hint: String) {
        let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
        self.pending = Some(slot.clone());
        self.error = None;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = read_step_file(&path, &hint);
            if let Ok(mut g) = slot.lock() {
                *g = Some(result);
            }
            ctx.request_repaint();
        });
    }

    /// Takes in a finished read.
    pub fn poll(&mut self) {
        let Some(slot) = &self.pending else { return };
        let done = slot.lock().ok().and_then(|mut g| g.take());
        match done {
            Some(Ok(read)) => {
                self.read = Some(read);
                self.error = None;
                self.pending = None;
            }
            Some(Err(e)) => {
                self.read = None;
                self.error = Some(e);
                self.pending = None;
            }
            None => {}
        }
    }

    pub fn clear(&mut self) {
        self.pending = None;
        self.read = None;
        self.error = None;
    }
}

/// The enclosure the cabinet is made from: the pick with the STEP file
/// applied, or the STEP file alone when nothing is picked.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Effective {
    pub choice: Option<Choice>,
    pub article: ArticleCheck,
    pub conflicts: Vec<String>,
}

pub(super) fn effective(picked: Option<&Choice>, step: Option<&StepRead>) -> Effective {
    match (picked, step) {
        (Some(c), Some(s)) => {
            let a = apply_step(c, s);
            Effective {
                choice: Some(a.choice),
                article: a.article,
                conflicts: a.conflicts,
            }
        }
        (None, Some(s)) => Effective {
            choice: Some(choice_from_step(s)),
            article: ArticleCheck::Unknown,
            conflicts: Vec::new(),
        },
        (Some(c), None) => Effective {
            choice: Some(c.clone()),
            article: ArticleCheck::Unknown,
            conflicts: Vec::new(),
        },
        (None, None) => Effective {
            choice: None,
            article: ArticleCheck::Unknown,
            conflicts: Vec::new(),
        },
    }
}

/// The drop zone and "Choose file..." for the maker's STEP file, and what was
/// read from it. Returns a file picked with the button.
pub(super) fn show_step(
    ui: &mut egui::Ui,
    slot: &mut StepSlot,
    picked: Option<&Choice>,
    eff: &Effective,
) -> Option<std::path::PathBuf> {
    let p = ui_palette();
    let dragging = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
    let mut chosen = None;
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 120.0).max(120.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::hover());
        slot.drop_rect = Some(rect);
        let over = dragging
            && ui
                .ctx()
                .input(|i| i.pointer.latest_pos())
                .is_some_and(|pos| rect.contains(pos));
        ui.painter().rect(
            rect,
            6.0,
            if over { p.hover } else { p.sunken },
            Stroke::new(
                if over { 1.5 } else { 1.0 },
                if over {
                    p.border_strong
                } else {
                    p.border_subtle
                },
            ),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            if picked.is_some() {
                "Drop its STEP file here"
            } else {
                "Or drop the enclosure's STEP file here"
            },
            egui::FontId::proportional(13.0),
            p.text_secondary,
        );
        if ui.button("Choose file…").clicked() {
            chosen = crate::dialogs::FileDialog::new()
                .set_title("The enclosure's STEP file")
                .add_filter("STEP or ZIP", &["stp", "step", "zip"])
                .pick_file();
        }
    });
    if slot.is_reading() {
        ui.weak("Reading the file…");
    }
    if let Some(e) = &slot.error {
        ui.colored_label(p.error, e);
    }
    if let Some(r) = &slot.read {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{}  ·  {} x {} x {} mm",
                r.file_name, r.size[0], r.size[1], r.size[2]
            ));
            match &eff.article {
                ArticleCheck::Matches => {
                    ui.colored_label(p.success, "matches");
                }
                ArticleCheck::Differs { file } => {
                    ui.colored_label(
                        p.warning,
                        format!(
                            "file says {file}, you picked {}",
                            picked.map_or("", |c| c.spec.article.as_str())
                        ),
                    );
                }
                ArticleCheck::Unknown => {}
            }
            if !r.notes.is_empty() {
                novo_theme::widgets::help_mark(ui, &r.notes.join("\n"));
            }
        });
        for line in &eff.conflicts {
            ui.colored_label(p.warning, line);
        }
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_parts::CatalogPart;

    fn part(order: &str, kind: &str, w: f64) -> CatalogPart {
        let mut p = CatalogPart {
            manufacturer: "Rittal".into(),
            order_number: order.into(),
            ..CatalogPart::default()
        };
        p.data
            .insert(keys::BODY_KIND.into(), Value::Text(kind.into()));
        p.data.insert(keys::OUTER_WIDTH_MM.into(), Value::Float(w));
        p.data
            .insert(keys::OUTER_HEIGHT_MM.into(), Value::Float(1800.0));
        p.data
            .insert(keys::OUTER_DEPTH_MM.into(), Value::Float(500.0));
        p
    }

    #[test]
    fn only_enclosure_parts_become_cards_with_catalogue_numbers() {
        let mut catalog = Catalog::default();
        catalog.upsert(part("8000", "enclosure", 600.0));
        catalog.upsert(part("8806000", "mounting_plate", 500.0));
        let cards = from_parts(&catalog);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].title, "Rittal 8000");
        assert_eq!(cards[0].spec.width.unwrap().source, Source::Catalogue);
        assert!(cards[0].plate_is_default);
        assert_eq!(
            (cards[0].plate.width, cards[0].plate.height),
            (500.0, 1600.0)
        );
    }

    #[test]
    fn list_entries_are_typed_and_keep_their_plate() {
        let e = EnclosureSize {
            name: "Mine".into(),
            width: 600.0,
            height: 800.0,
            depth: 250.0,
            plate_width: 500.0,
            plate_height: 700.0,
            margin: 5.0,
            material: novo_layout::sizing::Material::Steel,
        };
        let cards = from_list(&[e]);
        assert_eq!(cards[0].spec.depth.unwrap().source, Source::Typed);
        assert_eq!(cards[0].plate.margin, 5.0);
        assert!(!cards[0].plate_is_default);
        assert_eq!(cards[0].list_name.as_deref(), Some("Mine"));
    }

    #[test]
    fn search_finds_by_article_and_size() {
        let mut catalog = Catalog::default();
        catalog.upsert(part("8455600", "enclosure", 600.0));
        let cards = from_parts(&catalog);
        assert!(cards[0].matches("8455"));
        assert!(cards[0].matches("600 x 1800"));
        assert!(!cards[0].matches("9999"));
    }

    #[test]
    fn the_detail_leads_with_the_estimate_sentence_and_the_summary_is_one_line() {
        let mut catalog = Catalog::default();
        catalog.upsert(part("8000", "enclosure", 600.0));
        let c = &from_parts(&catalog)[0];
        assert!(detail(c).starts_with("Estimate from catalogue data"));
        assert!(!summary(c).contains('\n'));
        assert!(summary(c).contains("plate 500 x 1600"));
    }

    #[test]
    fn a_list_entry_keeps_the_typed_words() {
        let e = EnclosureSize {
            name: "Mine".into(),
            width: 600.0,
            height: 800.0,
            depth: 250.0,
            plate_width: 500.0,
            plate_height: 700.0,
            margin: 5.0,
            material: novo_layout::sizing::Material::Steel,
        };
        assert!(state_lines(&from_list(&[e])[0])[0].contains("typed by you"));
    }

    #[test]
    fn state_lines_say_where_each_number_came_from() {
        let mut catalog = Catalog::default();
        catalog.upsert(part("8000", "enclosure", 600.0));
        let lines = state_lines(&from_parts(&catalog)[0]);
        assert!(lines[0].contains("from your parts"));
        assert!(lines.last().unwrap().contains("default"));
    }

    const TS600: &[u8] =
        include_bytes!("../../../../qa/fixtures/enclosure-step/TS-600x1800x500.stp");
    const TS800: &[u8] =
        include_bytes!("../../../../qa/fixtures/enclosure-step/TS-800x1800x500.stp");

    fn card() -> Choice {
        let mut catalog = Catalog::default();
        catalog.upsert(part("TS-600x1800x500", "enclosure", 600.0));
        from_parts(&catalog).remove(0)
    }

    #[test]
    fn a_matching_step_file_gives_the_sizes_a_step_source_and_keeps_the_hash() {
        let c = card();
        let r = read_step_bytes("TS-600x1800x500.stp", TS600, &c.size_text()).unwrap();
        assert_eq!(r.size, [600.0, 1800.0, 500.0]);
        assert!(r.order_known);
        assert_eq!(r.sha256.len(), 64);
        let a = apply_step(&c, &r);
        assert_eq!(a.article, ArticleCheck::Matches);
        assert!(a.conflicts.is_empty(), "{:?}", a.conflicts);
        let spec = &a.choice.spec;
        assert_eq!(spec.width.unwrap().source, Source::Step);
        assert_eq!(spec.depth.unwrap().value, 500.0);
        assert_eq!(spec.step_name, "TS-600x1800x500.stp");
        assert_eq!(spec.step_sha256, r.sha256);
        // The plate is still a default of the STEP size, not the maker's.
        assert!(a.choice.plate_is_default);
        assert_eq!(
            (a.choice.plate.width, a.choice.plate.height),
            (500.0, 1600.0)
        );
    }

    #[test]
    fn another_articles_file_shows_both_numbers_and_the_step_wins() {
        let c = card();
        let r = read_step_bytes("TS-800x1800x500.stp", TS800, &c.size_text()).unwrap();
        let a = apply_step(&c, &r);
        assert!(
            matches!(a.article, ArticleCheck::Differs { .. }),
            "{:?}",
            a.article
        );
        assert_eq!(
            a.conflicts,
            vec!["Width: file 800 mm, your parts 600 mm".to_owned()]
        );
        assert_eq!(a.choice.spec.width.unwrap().value, 800.0);
    }

    #[test]
    fn a_file_alone_makes_an_enclosure_and_other_files_are_refused() {
        let r = read_step_bytes("TS-600x1800x500.stp", TS600, "").unwrap();
        let c = choice_from_step(&r);
        assert_eq!(c.origin, "Your STEP file");
        assert_eq!(c.spec.height.unwrap().source, Source::Step);
        assert!(read_step_bytes("notes.txt", b"hello", "").is_err());
        assert!(read_step_bytes("x.stp", b"not a step", "").is_err());
        // A ZIP with the file inside reads the same.
        let mut zip = Vec::new();
        {
            use std::io::Write;
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut zip));
            w.start_file(
                "sub/TS-600x1800x500.stp",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            w.write_all(TS600).unwrap();
            w.finish().unwrap();
        }
        let z = read_step_bytes("download.zip", &zip, "").unwrap();
        assert_eq!(z.size, r.size);
        assert_eq!(z.file_name, "TS-600x1800x500.stp");
        assert!(read_step_bytes("empty.zip", &empty_zip(), "").is_err());
    }

    fn empty_zip() -> Vec<u8> {
        let mut zip = Vec::new();
        zip::ZipWriter::new(std::io::Cursor::new(&mut zip))
            .finish()
            .unwrap();
        zip
    }
}
