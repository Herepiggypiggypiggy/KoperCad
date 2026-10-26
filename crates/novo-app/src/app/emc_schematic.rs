//! EMC net classes on the schematic (View > EMC net classes, Ctrl+K "EMC
//! net classes"). Every wire of the page is drawn in the colour of its
//! cable class (mains, motor, control, analog, bus, safety, earth), with a
//! small tag on the wire so the class never rests on colour alone. The class
//! is the one the EMC check uses: set by hand (`emc.class`), read from the
//! drawing (drive output, safety device, voltage) or guessed when the
//! drawing says nothing, and the card of a picked wire says which and why.
//!
//! A design aid: a class is a reading of the drawing, not a measurement.
//! Setting a class here is one undo step and changes what the EMC check
//! judges.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui;
use novo_canvas::model::WireLook;
use novo_core::{AnyId, PageId, Project, WireId};
use novo_emc::{EmcClass, Fix, Origin, Shield, Termination};
use novo_view3d::emc::Class;

use super::NovoApp;
use super::emc_adapter::class_of;
use super::emc_view::{style, text_on};

/// Panel width.
const WIDTH: f32 = 290.0;
/// A wire shorter than this on screen gets no tag.
const TAG_MIN_LEN: f32 = 40.0;

/// What the drawing says about one wire.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Info {
    pub class: Class,
    pub emc: EmcClass,
    pub origin: Origin,
    pub reason: String,
    pub name: String,
    pub shielded: bool,
    pub cable: Option<u64>,
}

/// One finding of the EMC check that is about a cable.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CableFinding {
    pub rule: &'static str,
    pub level: novo_emc::Level,
    pub message: String,
    pub fixes: Vec<Fix>,
}

/// What the card of a cable shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CableInfo {
    pub tag: String,
    /// Cores that carry a wire or a signal, the shield core not counted.
    pub cores: usize,
    pub wires: usize,
    /// The shield and how its ends are made, when the cable has one.
    pub shield: Option<Shield>,
    pub twisted: bool,
    pub findings: Vec<CableFinding>,
}

fn fix_cable(fix: &Fix) -> Option<u64> {
    match fix {
        Fix::AddShield { cable }
        | Fix::Terminate360 { cable }
        | Fix::ShortenPigtail { cable, .. } => Some(*cable),
        _ => None,
    }
}

/// What a finding's level means, in a few words beside it.
pub(super) fn level_short(level: novo_emc::Level) -> &'static str {
    match level {
        novo_emc::Level::Fact => "holds or it does not",
        novo_emc::Level::Index => "from geometry, for ordering",
        novo_emc::Level::Estimate => "an estimate, not a measurement",
        novo_emc::Level::Field => "a field model, not a measurement",
    }
}

/// The same in a sentence, for the hover.
pub(super) fn level_about(level: novo_emc::Level) -> &'static str {
    match level {
        novo_emc::Level::Fact => "Rule: a rule on what is drawn. It holds or it does not.",
        novo_emc::Level::Index => {
            "Index: a number from the geometry alone. Good for ordering, not a measurement."
        }
        novo_emc::Level::Estimate => {
            "Estimate: a closed-form estimate with the assumptions listed. Not a measurement."
        }
        novo_emc::Level::Field => {
            "Field: a modelled low-frequency field with its band and inputs. Not a measurement, and not valid outside its band."
        }
    }
}

/// An end of a shield in words.
pub(super) fn termination_word(t: Termination) -> String {
    match t {
        Termination::Full360 => "bonded all round".to_owned(),
        Termination::Pigtail(mm) => format!("on a pigtail of {} mm", mm.round() as i64),
        Termination::Open => "not connected".to_owned(),
        Termination::Unknown => "not stated".to_owned(),
    }
}

/// The class of every wire of the project, once per revision.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Data {
    pub wires: HashMap<u64, Info>,
    pub cables: HashMap<u64, CableInfo>,
}

impl Data {
    /// Reads the classes the way the EMC check does.
    pub(super) fn of(project: &Project) -> Data {
        let netlist = project.netlist();
        let input = novo_emc::project::input_of(project, &netlist);
        let wires = input
            .runs
            .iter()
            .map(|r| {
                (
                    r.id,
                    Info {
                        class: class_of(r.class),
                        emc: r.class,
                        origin: r.origin,
                        reason: r.class_reason.clone(),
                        name: r.name.clone(),
                        shielded: r.shield.is_some(),
                        cable: r.cable,
                    },
                )
            })
            .collect();
        let mut cables: HashMap<u64, CableInfo> = HashMap::new();
        for r in &input.runs {
            let Some(c) = r.cable else { continue };
            let cable = project.cable(novo_core::CableId::from(c));
            let e = cables.entry(c).or_insert_with(|| CableInfo {
                tag: cable.map_or_else(|| format!("cable {c}"), |k| k.tag.to_string()),
                cores: cable.map_or(0, |k| k.cores.iter().filter(|x| !x.shield).count()),
                wires: 0,
                shield: None,
                twisted: false,
                findings: Vec::new(),
            });
            e.wires += 1;
            e.shield = e.shield.or(r.shield);
            e.twisted |= r.twisted;
        }
        let report = novo_emc::analyse(&input);
        for f in &report.findings {
            for (c, info) in cables.iter_mut() {
                let about = f.subjects.contains(&novo_emc::Subject::Cable(*c))
                    || f.fixes.iter().any(|x| fix_cable(x) == Some(*c));
                if about {
                    info.findings.push(CableFinding {
                        rule: f.rule,
                        level: f.level,
                        message: f.message.clone(),
                        fixes: f.fixes.clone(),
                    });
                }
            }
        }
        Data { wires, cables }
    }

    /// Wires of `page` per class, and how many of them are guessed.
    pub(super) fn counts(&self, project: &Project, page: PageId) -> ([usize; 7], usize) {
        let mut counts = [0; 7];
        let mut guessed = 0;
        for w in project.wires().filter(|w| w.page == page) {
            if let Some(i) = self.wires.get(&w.id.raw()) {
                counts[i.class.index()] += 1;
                guessed += usize::from(i.origin == Origin::Guessed);
            }
        }
        (counts, guessed)
    }
}

/// What the schematic keeps of the net class overlay between frames.
#[derive(Default)]
pub(crate) struct EmcSchematic {
    pub(super) on: bool,
    /// Only this class stays in colour; the others step back.
    pub(super) focus: Option<Class>,
    /// The control a lesson step points at inside the panel, which scrolls
    /// into view when it is drawn and whenever another wire is picked.
    pub(super) reveal: Option<&'static str>,
    cache: Option<(u64, Arc<Data>)>,
}

impl EmcSchematic {
    /// A new project: nothing cached or focused from the old one.
    pub(super) fn forget(&mut self) {
        self.cache = None;
        self.focus = None;
        self.reveal = None;
    }

    pub(super) fn data(&mut self, project: &Project) -> Arc<Data> {
        let key = project.revision();
        if self.cache.as_ref().map(|c| c.0) != Some(key) {
            self.cache = Some((key, Arc::new(Data::of(project))));
        }
        Arc::clone(&self.cache.as_ref().expect("just filled").1)
    }
}

fn color(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

fn width_of(class: Class, guessed: bool) -> f32 {
    let base = match class {
        Class::Motor => 3.4,
        Class::Mains => 3.0,
        Class::Safety => 2.6,
        _ => 2.2,
    };
    if guessed { base - 0.6 } else { base }
}

fn origin_word(origin: Origin) -> &'static str {
    match origin {
        Origin::Declared => "set by you",
        Origin::Derived => "read from the drawing",
        Origin::Guessed => "guessed: the drawing gives no evidence",
    }
}

/// The wire looks of one page: class colour and weight, the focused class
/// in colour and the rest stepped back, the picked wire heavier.
pub(super) fn looks(
    project: &Project,
    page: PageId,
    data: &Data,
    focus: Option<Class>,
    selected: &[WireId],
    dark: bool,
) -> HashMap<WireId, WireLook> {
    let st = style(dark);
    let back = crate::theme::ui_palette()
        .text_secondary
        .gamma_multiply(0.45);
    project
        .wires()
        .filter(|w| w.page == page)
        .filter_map(|w| {
            let info = data.wires.get(&w.id.raw())?;
            let guessed = info.origin == Origin::Guessed;
            let (col, mut width) = if focus.is_none_or(|f| f == info.class) {
                let c = color(st.colour(info.class));
                (
                    if guessed { c.gamma_multiply(0.8) } else { c },
                    width_of(info.class, guessed),
                )
            } else {
                (back, 1.3)
            };
            if selected.contains(&w.id) {
                width += 1.6;
            }
            let [r, g, b, a] = col.to_srgba_unmultiplied();
            Some((
                w.id,
                WireLook {
                    color: novo_canvas::Color::rgba(r, g, b, a),
                    width,
                },
            ))
        })
        .collect()
}

/// The longest segment of a polyline and its length.
fn longest(points: &[egui::Pos2]) -> Option<(egui::Pos2, egui::Pos2, f32)> {
    points
        .windows(2)
        .map(|w| (w[0], w[1], w[0].distance(w[1])))
        .max_by(|a, b| a.2.total_cmp(&b.2))
}

impl NovoApp {
    /// `view.emc_nets`: the net class overlay on or off.
    pub(super) fn toggle_emc_nets(&mut self) {
        if self.workspace() != super::workspace::Workspace::Schematic && self.is_open() {
            self.switch_workspace(super::workspace::Workspace::Schematic);
        }
        self.wb.emc_sch.on ^= true;
        if self.wb.emc_sch.on {
            self.message(
                super::Level::Info,
                "EMC net classes: each wire in the colour of its class. A design aid read from the drawing; pick a wire to see why and to change it",
            );
        } else {
            self.wb.emc_sch.focus = None;
            self.light_wires(HashMap::new());
        }
    }

    /// Sets (or clears) the class of a wire by hand: one undo step.
    fn emc_set_class(&mut self, wire: WireId, class: Option<EmcClass>) {
        match class {
            Some(class) => {
                let fix = Fix::DeclareClass {
                    wire: wire.raw(),
                    class,
                };
                self.emc_apply(&fix);
            }
            None => {
                self.edit_project("EMC: back to automatic class", |tx| {
                    tx.set_property(wire, "emc.class", None)
                });
            }
        }
    }

    /// Paints the net class overlay over the schematic and shows its panel.
    pub(super) fn emc_schematic(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.wb.emc_sch.on {
            return;
        }
        let dark = ctx.global_style().visuals.dark_mode;
        let (Some(page), Some(doc)) = (self.current_page, self.backend.document()) else {
            return;
        };
        let Some(view) = self.canvas.schematic_ref() else {
            return;
        };
        let viewport = view.canvas.viewport;
        let project = doc.project();
        let data = self.wb.emc_sch.data(project);
        let focus = self.wb.emc_sch.focus;
        let live = self.wb.live.on;
        // A whole wire (double click) or a corner or run of one (a click).
        let mut picked: Vec<WireId> = self
            .selection()
            .into_iter()
            .filter_map(|id| match id {
                AnyId::Wire(w) => Some(w),
                _ => None,
            })
            .collect();
        if let Some(w) = view.picked_wire_part() {
            picked.push(w);
        }
        let (counts, guessed) = data.counts(project, page);
        let total: usize = counts.iter().sum();

        if !live {
            let wire_looks = looks(project, page, &data, focus, &picked, dark);
            let to_screen = |p: egui::Vec2| {
                let s = viewport.world_to_screen(novo_canvas::Vec2::new(p.x, p.y));
                canvas.min + egui::vec2(s.x, s.y)
            };
            let st = style(dark);
            let painter = ctx
                .layer_painter(egui::LayerId::new(
                    egui::Order::Background,
                    egui::Id::new("emc_net_tags"),
                ))
                .with_clip_rect(canvas);
            let font = egui::FontId::proportional(10.5);
            let mut placed: Vec<egui::Rect> = Vec::new();
            for w in project.wires().filter(|w| w.page == page) {
                let Some(info) = data.wires.get(&w.id.raw()) else {
                    continue;
                };
                if focus.is_some_and(|f| f != info.class) {
                    continue;
                }
                let (Some(a), Some(b)) = (
                    project.endpoint_position(w.from),
                    project.endpoint_position(w.to),
                ) else {
                    continue;
                };
                let mut pts = vec![egui::vec2(a.x as f32, a.y as f32)];
                pts.extend(w.route.iter().map(|p| egui::vec2(p.x as f32, p.y as f32)));
                pts.push(egui::vec2(b.x as f32, b.y as f32));
                let screen: Vec<egui::Pos2> = pts.into_iter().map(to_screen).collect();
                let Some((p, q, len)) = longest(&screen) else {
                    continue;
                };
                if len < TAG_MIN_LEN {
                    continue;
                }
                let mid = p + (q - p) * 0.5;
                let text = format!(
                    "{}{}",
                    info.class.short(),
                    if info.origin == Origin::Guessed {
                        " ?"
                    } else {
                        ""
                    }
                );
                let galley = painter.layout_no_wrap(text, font.clone(), egui::Color32::WHITE);
                let pill = egui::Rect::from_center_size(mid, galley.size() + egui::vec2(8.0, 3.0));
                if !canvas.contains_rect(pill)
                    || placed.iter().any(|r| r.intersects(pill.expand(2.0)))
                {
                    continue;
                }
                placed.push(pill);
                let bg = st.colour(info.class);
                painter.rect_filled(pill, 7.0, color(bg));
                painter.galley(pill.center() - galley.size() * 0.5, galley, text_on(bg));
            }
            self.light_wires(wire_looks);
        }

        // The panel: honesty line, the classes with counts (a click keeps
        // one class in colour), and the picked wire's card.
        let st = style(dark);
        let pal = crate::theme::ui_palette();
        let mut focus_to = focus;
        let mut set: Option<(WireId, Option<EmcClass>)> = None;
        let mut apply: Option<Fix> = None;
        let mut want_preview: Option<Fix> = None;
        let mut cancel_preview = false;
        let mut show_view: Option<super::workspace::Workspace> = None;
        let revision = self
            .backend
            .document()
            .map_or(0, |d| d.project().revision());
        let shown = self.emc.shown_preview(revision);
        let preview_fresh = self.emc.take_fresh();
        let mut off = false;
        // Controls the lessons point at: (anchor, where, label, clicked this frame).
        let mut marks: Vec<(&'static str, egui::Rect, &'static str, bool)> = Vec::new();
        let card = picked
            .first()
            .and_then(|w| data.wires.get(&w.raw()).map(|info| (*w, info.clone())));
        // A newly picked wire scrolls its card into view: with a lesson card
        // or a small window the panel is short, and the card is the point of
        // picking a wire.
        let reveal = self.wb.emc_sch.reveal;
        let revealed = egui::Id::new("emc_nets_revealed");
        let scroll_reveal = reveal.is_some()
            && (ctx.data(|d| d.get_temp::<Option<&'static str>>(revealed)) != Some(reveal));
        ctx.data_mut(|d| d.insert_temp(revealed, reveal));
        let seen = egui::Id::new("emc_nets_card_seen");
        let now = card.as_ref().map(|c| c.0);
        let fresh = now.is_some() && ctx.data(|d| d.get_temp::<Option<WireId>>(seen)) != Some(now);
        let scroll_reveal = scroll_reveal || (fresh && reveal.is_some());
        ctx.data_mut(|d| d.insert_temp(seen, now));
        super::panel::Panel::new(
            "schematic_emc_nets",
            "EMC net classes",
            canvas,
            super::panel::Side::Right,
            WIDTH,
        )
        .above_card()
        .fit()
        .scrolls()
        .show(ctx, |ui| {
            // The panel stays this wide when a fix preview opens (its long
            // lines wrap) instead of growing over the drawing.
            ui.set_max_width(WIDTH - 24.0);
            ui.label(
                egui::RichText::new(
                    "Indicative: classes are read from the drawing. A design aid, not a test result.",
                )
                .small()
                .color(pal.text_secondary),
            );
            if live {
                ui.label(
                    egui::RichText::new(
                        "The live view is on and colours the wires by potential. Switch it off (Shift+L) to see the classes.",
                    )
                    .small()
                    .color(pal.warning),
                );
            }
            if total == 0 {
                ui.label("No wires on this page.");
            }
            for class in Class::ALL {
                let n = counts[class.index()];
                if n == 0 {
                    continue;
                }
                let on = focus_to == Some(class);
                let row = ui
                    .horizontal(|ui| {
                        let (r, _) =
                            ui.allocate_exact_size(egui::vec2(26.0, 10.0), egui::Sense::hover());
                        ui.painter().rect_filled(r, 5.0, color(st.colour(class)));
                        ui.selectable_label(on, format!("{}  ({n})", class.name()))
                    })
                    .inner;
                let row = row.on_hover_text(
                    "Keep this class in colour and step the others back; click again for all",
                );
                let hit = row.clicked();
                if hit {
                    focus_to = if on { None } else { Some(class) };
                }
                if class == Class::Motor {
                    marks.push(("emc.nets.motor", row.rect, "Motor and drive output", hit));
                }
            }
            if guessed > 0 {
                ui.label(
                    egui::RichText::new(format!(
                        "{guessed} wire{} marked ? {} guessed: nothing in the drawing says what {} carry. Pick one and set its class.",
                        if guessed == 1 { "" } else { "s" },
                        if guessed == 1 { "is" } else { "are" },
                        if guessed == 1 { "it" } else { "they" },
                    ))
                    .small()
                    .color(pal.warning),
                );
            }
            if let Some((wire, info)) = &card {
                ui.separator();
                let title = ui.label(egui::RichText::new(format!("Wire {}", info.name)).strong());
                if fresh && reveal.is_none() {
                    title.scroll_to_me(Some(egui::Align::TOP));
                }
                ui.label(format!(
                    "{}{}",
                    info.class.name(),
                    if info.shielded { ", shielded cable" } else { "" }
                ));
                ui.label(
                    egui::RichText::new(if info.origin == Origin::Declared {
                        origin_word(info.origin).to_owned()
                    } else {
                        format!("{}: {}", origin_word(info.origin), info.reason)
                    })
                        .small()
                        .color(pal.text_secondary),
                );
                if let Some(cable) = info.cable.and_then(|c| data.cables.get(&c)) {
                    ui.separator();
                    ui.label(egui::RichText::new(format!("Cable {}", cable.tag)).strong());
                    ui.label(format!(
                        "{} core{}, {} wire{} drawn, {}",
                        cable.cores,
                        if cable.cores == 1 { "" } else { "s" },
                        cable.wires,
                        if cable.wires == 1 { "" } else { "s" },
                        if cable.twisted { "twisted" } else { "not stated as twisted" },
                    ));
                    match &cable.shield {
                        Some(sh) => ui.label(format!(
                            "Shield: end A {}, end B {}",
                            termination_word(sh.a),
                            termination_word(sh.b)
                        )),
                        None => ui.label("No shield core"),
                    };
                    for f in &cable.findings {
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{}  {}: {}",
                                f.rule,
                                f.level.label(),
                                level_short(f.level)
                            ))
                            .small()
                            .strong(),
                        )
                        .on_hover_text(level_about(f.level));
                        ui.label(egui::RichText::new(&f.message).small());
                        for fix in &f.fixes {
                            if fix_cable(fix).is_none() {
                                continue;
                            }
                            if fix.applicable() {
                                let open = shown.as_ref().is_some_and(|(f, _)| f == fix);
                                let button = ui
                                    .selectable_label(open, format!("Preview: {}", fix.hint()))
                                    .on_hover_text("See what it clears before anything changes; Apply is one undo step");
                                if marks.iter().all(|m| m.0 != "emc.nets.preview") {
                                    marks.push(("emc.nets.preview", button.rect, "Preview", button.clicked()));
                                    if scroll_reveal && reveal == Some("emc.nets.preview") {
                                        button.scroll_to_me(Some(egui::Align::Center));
                                    }
                                }
                                if button.clicked() {
                                    if open {
                                        cancel_preview = true;
                                    } else {
                                        want_preview = Some(fix.clone());
                                    }
                                }
                                if let Some((pf, pv)) = &shown
                                    && pf == fix
                                {
                                    let mut apply_at = None;
                                    let applied = super::emc::preview_card(
                                        ui,
                                        pv,
                                        &mut cancel_preview,
                                        &mut show_view,
                                        preview_fresh,
                                        &mut apply_at,
                                        &mut None,
                                    );
                                    if let Some(rect) = apply_at
                                        && marks.iter().all(|m| m.0 != "emc.nets.apply")
                                    {
                                        marks.push(("emc.nets.apply", rect, "Apply", applied));
                                        if scroll_reveal && reveal == Some("emc.nets.apply") {
                                            ui.scroll_to_rect(rect, Some(egui::Align::Center));
                                        }
                                    }
                                    if applied {
                                        apply = Some(pf.clone());
                                    }
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new(format!("By hand: {}", fix.hint()))
                                        .small()
                                        .color(pal.text_secondary),
                                );
                            }
                        }
                    }
                    if cable.findings.is_empty() {
                        ui.label(
                            egui::RichText::new("No finding about this cable. That is not a statement that it is fine.")
                                .small()
                                .color(pal.text_secondary),
                        );
                    }
                    ui.label(
                        egui::RichText::new("Indicative: a design aid, not a test result.")
                            .small()
                            .color(pal.text_secondary),
                    );
                }
                ui.separator();
                ui.label(egui::RichText::new("Set the class").small().strong());
                ui.horizontal_wrapped(|ui| {
                    for c in EmcClass::ALL {
                        let mine = class_of(c) == info.class && info.origin == Origin::Declared;
                        let chip = ui.selectable_label(mine, class_of(c).short());
                        if chip.clicked() {
                            set = Some((*wire, Some(c)));
                        }
                        if class_of(c) == Class::Control {
                            marks.push(("emc.nets.set_control", chip.rect, "control", chip.clicked()));
                            if scroll_reveal && reveal == Some("emc.nets.set_control") {
                                chip.scroll_to_me(Some(egui::Align::Center));
                            }
                        }
                    }
                });
                if info.origin == Origin::Declared
                    && ui
                        .small_button("Back to automatic")
                        .on_hover_text("Remove the class set by hand")
                        .clicked()
                {
                    set = Some((*wire, None));
                }
                ui.label(
                    egui::RichText::new("One undo step. Check > EMC check uses this class.")
                        .small()
                        .color(pal.text_secondary),
                );
            } else if total > 0 {
                ui.label(
                    egui::RichText::new("Pick a wire to see why it has its class. Check > EMC check uses these classes.")
                        .small()
                        .color(pal.text_secondary),
                );
            }
            ui.add_space(2.0);
            let close = ui.small_button("Close");
            marks.push(("emc.nets.close", close.rect, "Close", close.clicked()));
            if scroll_reveal && reveal == Some("emc.nets.close") {
                close.scroll_to_me(Some(egui::Align::Center));
            }
            if close.clicked() {
                off = true;
            }
        });
        for (id, rect, label, clicked) in marks {
            self.anchor_clicked(id, rect, label, clicked);
        }
        self.wb.emc_sch.focus = focus_to;
        if let Some((wire, class)) = set {
            self.emc_set_class(wire, class);
        }
        self.emc_preview_actions(want_preview, cancel_preview, show_view, apply);
        if off {
            self.toggle_emc_nets();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::gate1_tests::Drive;

    fn example() -> (tempfile::TempDir, Drive) {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run_command(&ctx, "file.example_emc", None);
        d.frame();
        (dir, d)
    }

    fn data(d: &Drive) -> Data {
        Data::of(d.app.backend.document().unwrap().project())
    }

    #[test]
    fn the_example_has_mains_analog_and_earth_wires_and_some_guesses() {
        let (_dir, d) = example();
        let data = data(&d);
        let has = |c: Class| data.wires.values().any(|i| i.class == c);
        assert!(has(Class::Mains) && has(Class::Analog) && has(Class::Earth));
        assert!(data.wires.values().any(|i| i.origin == Origin::Guessed));
        assert!(data.wires.values().all(|i| !i.reason.is_empty()));
    }

    #[test]
    fn the_command_switches_the_overlay_on_and_off_and_draws_without_a_panic() {
        let (_dir, mut d) = example();
        let ctx = d.ctx.clone();
        d.app.run_command(&ctx, "view.emc_nets", None);
        assert!(d.app.wb.emc_sch.on);
        for _ in 0..3 {
            d.frame();
        }
        d.app.run_command(&ctx, "view.emc_nets", None);
        assert!(!d.app.wb.emc_sch.on);
        d.frame();
    }

    #[test]
    fn a_focused_class_keeps_its_colour_and_the_others_step_back() {
        let (_dir, d) = example();
        let project = d.app.backend.document().unwrap().project();
        let page = project.wires().next().unwrap().page;
        let data = data(&d);
        let all = looks(project, page, &data, None, &[], true);
        let motor = looks(project, page, &data, Some(Class::Mains), &[], true);
        assert_eq!(all.len(), motor.len());
        let mains = project
            .wires()
            .find(|w| w.page == page && data.wires[&w.id.raw()].class == Class::Mains)
            .unwrap();
        let other = project
            .wires()
            .find(|w| w.page == page && data.wires[&w.id.raw()].class != Class::Mains)
            .unwrap();
        assert_eq!(all[&mains.id], motor[&mains.id]);
        assert!(motor[&other.id].width < all[&other.id].width);
    }

    #[test]
    fn a_new_project_does_not_get_the_old_projects_classes_or_focus() {
        let (_dir, mut d) = example();
        let ctx = d.ctx.clone();
        d.app.run_command(&ctx, "view.emc_nets", None);
        d.frame();
        d.app.wb.emc_sch.focus = Some(Class::Mains);
        let old = {
            let project = d.app.backend.document().unwrap().project();
            d.app.wb.emc_sch.data(project).wires.len()
        };
        assert!(old > 0);
        d.app.run(&ctx, crate::actions::Action::NewProject);
        d.frame();
        assert!(d.app.wb.emc_sch.on, "the overlay stays on");
        assert_eq!(d.app.wb.emc_sch.focus, None);
        let project = d.app.backend.document().unwrap().project();
        let cached = d.app.wb.emc_sch.data(project);
        assert_eq!(*cached, Data::of(project));
    }

    #[test]
    fn setting_a_class_is_one_undo_step_and_back_to_automatic_restores_the_guess() {
        let (_dir, mut d) = example();
        let (wire, was) = {
            let data = data(&d);
            let (id, info) = data
                .wires
                .iter()
                .find(|(_, i)| i.origin == Origin::Guessed)
                .map(|(id, i)| (*id, i.clone()))
                .unwrap();
            (WireId::from(id), info)
        };
        let depth = d.app.backend.document().unwrap().undo_depth();
        d.app.emc_set_class(wire, Some(EmcClass::Analog));
        d.frame();
        let now = data(&d).wires[&wire.raw()].clone();
        assert_eq!((now.class, now.origin), (Class::Analog, Origin::Declared));
        assert_eq!(d.app.backend.document().unwrap().undo_depth(), depth + 1);
        d.app.emc_set_class(wire, None);
        d.frame();
        let back = data(&d).wires[&wire.raw()].clone();
        assert_eq!((back.class, back.origin), (was.class, Origin::Guessed));
    }

    #[test]
    fn the_motor_cable_has_a_card_and_its_shield_fix_is_one_undo_step() {
        let (_dir, mut d) = example();
        let data = data(&d);
        let (cable, info) = data
            .cables
            .iter()
            .find(|(_, c)| {
                c.findings
                    .iter()
                    .any(|f| f.fixes.iter().any(|x| matches!(x, Fix::AddShield { .. })))
            })
            .map(|(c, i)| (*c, i.clone()))
            .unwrap_or_else(|| panic!("no cable with a shield finding: {:#?}", data.cables));
        assert!(info.shield.is_none() && info.wires >= 1 && info.cores >= 1);
        let depth = d.app.backend.document().unwrap().undo_depth();
        d.app.emc_apply(&Fix::AddShield { cable });
        d.frame();
        assert_eq!(d.app.backend.document().unwrap().undo_depth(), depth + 1);
        let after = Data::of(d.app.backend.document().unwrap().project());
        assert!(
            after.cables[&cable]
                .findings
                .iter()
                .all(|f| !f.fixes.iter().any(|x| matches!(x, Fix::AddShield { .. }))),
            "the shield finding stays after the fix"
        );
    }

    #[test]
    fn the_cable_card_previews_a_fix_before_it_is_applied() {
        let (_dir, mut d) = example();
        let data = data(&d);
        let cable = data
            .cables
            .iter()
            .find(|(_, c)| {
                c.findings
                    .iter()
                    .any(|f| f.fixes.iter().any(|x| matches!(x, Fix::AddShield { .. })))
            })
            .map(|(c, _)| *c)
            .expect("a cable with a shield finding");
        let fix = Fix::AddShield { cable };
        let doc = |d: &Drive| d.app.backend.document().unwrap().project().revision();
        let (revision, depth) = (doc(&d), d.app.backend.document().unwrap().undo_depth());
        // The card asks for the preview; nothing changes.
        d.app
            .emc_preview_actions(Some(fix.clone()), false, None, None);
        d.frame();
        assert_eq!(
            (doc(&d), d.app.backend.document().unwrap().undo_depth()),
            (revision, depth)
        );
        let (shown, result) = d.app.emc.shown_preview(revision).expect("a preview");
        assert_eq!(shown, fix);
        // The finding stays as an info (ends not stated), so the preview says what it does.
        assert!(result.unwrap().said.contains("Shield core added"));
        // Cancel drops it; the card can ask again and apply it as one step.
        d.app.emc_preview_actions(None, true, None, None);
        assert!(d.app.emc.shown_preview(revision).is_none());
        d.app
            .emc_preview_actions(Some(fix.clone()), false, None, None);
        d.app.emc_preview_actions(None, false, None, Some(fix));
        d.frame();
        assert_eq!(d.app.backend.document().unwrap().undo_depth(), depth + 1);
        assert!(d.app.emc.shown_preview(doc(&d)).is_none());
    }

    #[test]
    fn a_picked_wire_shows_its_card_and_the_lesson_controls_without_a_selection() {
        let (_dir, mut d) = example();
        let ctx = d.ctx.clone();
        d.app.run_command(&ctx, "view.emc_nets", None);
        d.frame();
        d.frame();
        d.app.solve_pick_wire("-U1:U~-X2:1").unwrap();
        d.frame();
        d.frame();
        assert!(
            d.app.selection().is_empty(),
            "a pick is a click on the wire, not a selection that opens the Inspector"
        );
        for id in [
            "emc.nets.motor",
            "emc.nets.preview",
            "emc.nets.set_control",
            "emc.nets.close",
        ] {
            assert!(d.app.learn_drawn(id).is_some(), "{id} is not drawn");
        }
    }

    #[test]
    fn every_level_has_a_short_meaning_and_a_hover_that_starts_with_its_label() {
        for level in [
            novo_emc::Level::Fact,
            novo_emc::Level::Index,
            novo_emc::Level::Estimate,
            novo_emc::Level::Field,
        ] {
            assert!(!level_short(level).is_empty());
            assert!(level_about(level).starts_with(level.label()));
        }
    }
}
