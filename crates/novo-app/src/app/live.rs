//! The live potentials view (point 22, display side): Shift+L turns the
//! drawing into a bench where every switched pole can be closed or opened
//! and the wires show what they carry under that state, as solved by
//! novo-sim. Nothing here touches the model: the states live in the
//! workbench and the wires are lit in the canvas's own wire layer.
//!
//! Poles start open (a plant at rest). A badge sits on every switched pole of
//! the page: click it to close or open that pole, Shift+click to switch every
//! pole of the symbol. A wire carrying one potential is lit in that
//! potential's colour, wires nothing reaches are dimmed, and a net where two
//! potentials meet is drawn red.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use eframe::egui;
use novo_canvas::model::WireLook;
use novo_core::{
    NetId, Netlist, PageId, Potential, PotentialId, PotentialKind, Project, SymbolId, SymbolRole,
    WireId,
};
use novo_sim::{Clock, Live, PoleState, Settled, Solution, is_coil, settle_timed};
use novo_theme::{tokens, widgets};

use super::live_anim::Anim;
use super::{Level, NovoApp};

/// Radius of a pole badge, in screen points.
const BADGE: f32 = 6.0;
/// How close the pointer must be to a wire to describe it, in screen points.
const HOVER_REACH: f32 = 5.0;
/// Width of the glow on a lit wire, in screen points.
const LIT_WIDTH: f32 = 4.5;

/// What the live view remembers.
#[derive(Default)]
pub(crate) struct LiveView {
    pub(super) on: bool,
    /// Poles closed by hand. A contact whose device draws a coil follows the
    /// coil instead; a hand-set state only adds to it.
    pub(super) poles: PoleState,
    /// Coils switched on by hand, whatever feeds them.
    pub(super) forced: BTreeSet<SymbolId>,
    /// Seconds the bench has run, for timer contacts.
    clock: Clock,
    /// Easing state of the device animation (lamps, motors, coils).
    anim: Anim,
    solved: Option<Solved>,
}

/// A solution for one project revision and one pole state.
struct Solved {
    revision: u64,
    poles: PoleState,
    forced: BTreeSet<SymbolId>,
    netlist: Netlist,
    settled: Settled,
}

impl LiveView {
    /// The solution for `project` under the current poles, solved again only
    /// when the drawing or the poles changed.
    fn solved(&mut self, project: &Project, dt: f64) -> &Solved {
        let changed = self.solved.as_ref().is_none_or(|s| {
            s.revision != project.revision()
                || s.poles != self.poles
                || s.forced != self.forced
                || !s.netlist.is_current(project)
        });
        let counting = self.solved.as_ref().is_some_and(|s| s.settled.timing);
        if counting && !changed {
            // A timer is running: time passes and the contacts may follow.
            self.clock.advance(dt);
            if let Some(s) = self.solved.as_mut() {
                s.settled = settle_timed(
                    project,
                    &s.netlist,
                    &s.poles,
                    &s.forced,
                    &s.settled.energized,
                    &mut self.clock,
                );
            }
        } else if changed {
            let netlist = project.netlist();
            // The coils that were on stay on while their own contacts feed
            // them, so a seal-in circuit holds after the button is released.
            let held = self
                .solved
                .as_ref()
                .map(|s| s.settled.energized.clone())
                .unwrap_or_default();
            let settled = settle_timed(
                project,
                &netlist,
                &self.poles,
                &self.forced,
                &held,
                &mut self.clock,
            );
            self.solved = Some(Solved {
                revision: project.revision(),
                poles: self.poles.clone(),
                forced: self.forced.clone(),
                netlist,
                settled,
            });
        }
        self.solved.as_ref().expect("solved above")
    }

    /// Closes the pole if open and opens it if closed.
    pub(super) fn flip(&mut self, symbol: SymbolId, group: usize) {
        let closed = self.poles.is_closed(symbol, group);
        self.poles.set(symbol, group, !closed);
    }

    /// Flips every pole of a symbol to the opposite of its first pole.
    pub(super) fn flip_all(&mut self, project: &Project, symbol: SymbolId) {
        let closed = self.poles.is_closed(symbol, 0);
        self.poles.set_all(project, symbol, !closed);
    }
}

/// One wire of the page and what it carries.
pub(super) struct LitWire {
    pub id: WireId,
    pub points: Vec<egui::Vec2>,
    pub live: Live,
}

/// The wires of `page` as world polylines (mm) with their live state.
pub(super) fn lit_wires(
    project: &Project,
    page: PageId,
    netlist: &Netlist,
    solution: &Solution,
) -> Vec<LitWire> {
    project
        .wires()
        .filter(|w| w.page == page)
        .filter_map(|w| {
            let a = project.endpoint_position(w.from)?;
            let b = project.endpoint_position(w.to)?;
            let mut points = vec![egui::vec2(a.x as f32, a.y as f32)];
            points.extend(w.route.iter().map(|p| egui::vec2(p.x as f32, p.y as f32)));
            points.push(egui::vec2(b.x as f32, b.y as f32));
            let live = netlist
                .net_of_wire(w)
                .map_or(Live::Dead, |n| solution.net(n).clone());
            Some(LitWire {
                id: w.id,
                points,
                live,
            })
        })
        .collect()
}

/// Distance in screen points from `p` to the segment `a`-`b`.
fn distance_to_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        (((p - a).dot(ab)) / len2).clamp(0.0, 1.0)
    };
    (a + ab * t).distance(p)
}

/// A switched pole of the page, for its badge.
pub(super) struct PoleBadge {
    pub symbol: SymbolId,
    pub group: usize,
    /// Page position (mm) halfway between the pole's pins.
    pub at: egui::Vec2,
    pub closed: bool,
}

pub(super) fn pole_badges(project: &Project, page: PageId, poles: &PoleState) -> Vec<PoleBadge> {
    let mut badges = Vec::new();
    for symbol in project.symbols().filter(|s| s.page == page) {
        for (group, pins) in symbol.pass_through.iter().enumerate() {
            let at: Vec<_> = pins
                .iter()
                .filter_map(|&p| symbol.pin_position(p))
                .collect();
            if at.len() < 2 {
                continue;
            }
            let n = at.len() as f64;
            let x = at.iter().map(|p| p.x).sum::<f64>() / n;
            let y = at.iter().map(|p| p.y).sum::<f64>() / n;
            badges.push(PoleBadge {
                symbol: symbol.id,
                group,
                at: egui::vec2(x as f32, y as f32),
                closed: poles.is_closed(symbol.id, group),
            });
        }
    }
    badges
}

/// A coil of the page, for its badge.
pub(super) struct CoilBadge {
    pub symbol: SymbolId,
    pub at: egui::Vec2,
    pub on: bool,
}

pub(super) fn coil_badges(
    project: &Project,
    page: PageId,
    energized: &BTreeSet<SymbolId>,
) -> Vec<CoilBadge> {
    project
        .symbols()
        .filter(|s| s.page == page && is_coil(s))
        .filter_map(|s| {
            let a = s.pin_position(0)?;
            let b = s.pin_position(1)?;
            Some(CoilBadge {
                symbol: s.id,
                at: egui::vec2(((a.x + b.x) / 2.0) as f32, ((a.y + b.y) / 2.0) as f32),
                on: energized.contains(&s.id),
            })
        })
        .collect()
}

/// How a potential is drawn: its kind first, then its name for the line
/// conductors, so L1, L2 and L3 stay apart.
pub(super) fn potential_color(potential: &Potential) -> egui::Color32 {
    let palette = novo_theme::POTENTIALS_DARK;
    let color = |c| novo_theme::canvas::to_color32(c);
    match potential.kind {
        PotentialKind::ProtectiveEarth => color(palette.protective_earth),
        PotentialKind::Pen => color(palette.pen),
        PotentialKind::Neutral => color(palette.neutral),
        PotentialKind::DcPositive => color(palette.dc_positive),
        PotentialKind::DcNegative => color(palette.dc_negative),
        PotentialKind::Line | PotentialKind::Signal => {
            let slot = potential
                .name
                .chars()
                .rev()
                .find_map(|c| c.to_digit(10))
                .map_or_else(
                    || potential.name.bytes().map(usize::from).sum::<usize>(),
                    |d| d.saturating_sub(1) as usize,
                )
                % palette.line.len();
            color(palette.line[slot])
        }
    }
}

/// What the legend lists: each potential reaching the page, with how many
/// nets of it, and the nets where potentials meet.
#[derive(Default, Debug, PartialEq, Eq)]
pub(super) struct Legend {
    pub carried: BTreeMap<PotentialId, usize>,
    pub shorted: usize,
}

pub(super) fn legend(
    wires: &[LitWire],
    project: &Project,
    page: PageId,
    netlist: &Netlist,
    solution: &Solution,
) -> Legend {
    let mut seen: Vec<NetId> = Vec::new();
    let mut legend = Legend::default();
    for wire in project.wires().filter(|w| w.page == page) {
        let Some(net) = netlist.net_of_wire(wire) else {
            continue;
        };
        if seen.contains(&net) {
            continue;
        }
        seen.push(net);
        match solution.net(net) {
            Live::Dead => {}
            Live::Carries(p) => *legend.carried.entry(*p).or_default() += 1,
            Live::Shorted(_) => legend.shorted += 1,
        }
    }
    let _ = wires;
    legend
}

impl NovoApp {
    /// Shift+L: the live potentials view on or off.
    pub(super) fn toggle_live(&mut self) {
        self.wb.live.on ^= true;
        if self.wb.live.on {
            self.message(
                Level::Info,
                "Live potentials: contacts start open. Click a pole badge to close it",
            );
        }
    }

    /// Hands the canvas the wire looks of the live view (none when it is off).
    fn light_wires(&mut self, looks: HashMap<WireId, WireLook>) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        if let Some(view) = self.canvas.schematic() {
            view.set_wire_looks(doc.project(), looks);
        }
    }

    /// Paints the live view over the canvas and shows its card and badges.
    pub(super) fn live_view(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.wb.live.on {
            self.light_wires(HashMap::new());
            return;
        }
        let (Some(page), Some(doc)) = (self.current_page, self.backend.document()) else {
            return;
        };
        let Some(view) = self.canvas.schematic_ref() else {
            return;
        };
        let viewport = view.canvas.viewport;
        let to_screen = |p: egui::Vec2| {
            let s = viewport.world_to_screen(novo_canvas::Vec2::new(p.x, p.y));
            canvas.min + egui::vec2(s.x, s.y)
        };
        let project = doc.project();
        let live = &mut self.wb.live;
        let dt = f64::from(ctx.input(|i| i.stable_dt));
        let solved = live.solved(project, dt);
        let timing = solved.settled.timing.then_some(solved.settled.remaining);
        let wires = lit_wires(project, page, &solved.netlist, &solved.settled.solution);
        let legend = legend(
            &wires,
            project,
            page,
            &solved.netlist,
            &solved.settled.solution,
        );
        let badges = pole_badges(project, page, &solved.settled.poles);
        let driven = solved.settled.driven.clone();
        let energized = solved.settled.energized.clone();
        let coils = coil_badges(project, page, &energized);
        let colors: BTreeMap<PotentialId, (String, egui::Color32)> = project
            .potentials()
            .map(|p| (p.id, (p.name.clone(), potential_color(p))))
            .collect();
        let states = novo_sim::device_states(project, &solved.netlist, &solved.settled);
        let names: BTreeMap<SymbolId, String> = badges
            .iter()
            .map(|b| (b.symbol, symbol_name(project, b.symbol)))
            .collect();

        // Devices: lamps glow, motors turn, coils pull in.
        let animating = self.wb.live.anim.paint(
            ctx,
            canvas,
            viewport.zoom,
            &to_screen,
            project,
            self.canvas
                .schematic_ref()
                .map(|v| v.editor.catalog.library()),
            page,
            &states,
            &energized,
            dt as f32,
        );

        // Wires: lit in their colour, dimmed when nothing reaches them. The
        // canvas draws them in its wire layer, under the symbols and text.
        let looks = wires
            .iter()
            .map(|wire| {
                let (color, width) = match &wire.live {
                    Live::Dead => (tokens::BG_0.gamma_multiply(0.6), 2.5),
                    Live::Carries(p) => {
                        let color = colors.get(p).map_or(tokens::ACCENT, |c| c.1);
                        (color.gamma_multiply(0.9), LIT_WIDTH)
                    }
                    Live::Shorted(_) => (tokens::ERROR, LIT_WIDTH + 1.0),
                };
                let [r, g, b, a] = color.to_srgba_unmultiplied();
                (
                    wire.id,
                    WireLook {
                        color: novo_canvas::Color::rgba(r, g, b, a),
                        width,
                    },
                )
            })
            .collect();
        if let Some(view) = self.canvas.schematic() {
            view.set_wire_looks(project, looks);
        }

        // Hovering a wire says what it carries right now.
        if let Some(pointer) = ctx.pointer_hover_pos()
            && canvas.contains(pointer)
            && ctx
                .layer_id_at(pointer)
                .is_none_or(|l| l.order == egui::Order::Background)
        {
            let hit = wires.iter().find(|w| {
                w.points.windows(2).any(|seg| {
                    distance_to_segment(pointer, to_screen(seg[0]), to_screen(seg[1]))
                        <= HOVER_REACH
                })
            });
            if let Some(wire) = hit {
                let text = match &wire.live {
                    Live::Dead => "Carries nothing".to_string(),
                    Live::Carries(p) => {
                        format!("Carries {}", colors.get(p).map_or("?", |c| c.0.as_str()))
                    }
                    Live::Shorted(ps) => format!(
                        "Short: {}",
                        ps.iter()
                            .filter_map(|p| colors.get(p).map(|c| c.0.as_str()))
                            .collect::<Vec<_>>()
                            .join(" meets ")
                    ),
                };
                egui::Area::new(egui::Id::new("live_wire_tip"))
                    .order(egui::Order::Tooltip)
                    .interactable(false)
                    .fixed_pos(pointer + egui::vec2(14.0, 14.0))
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(text));
                    });
            }
        }

        // Pole badges: a click closes or opens the pole.
        let mut clicked: Option<(SymbolId, usize, bool)> = None;
        for badge in &badges {
            let at = to_screen(badge.at);
            if !canvas.expand(BADGE).contains(at) {
                continue;
            }
            let id = egui::Id::new(("live_pole", badge.symbol, badge.group));
            egui::Area::new(id)
                .order(egui::Order::Middle)
                .pivot(egui::Align2::CENTER_CENTER)
                .fixed_pos(at)
                .show(ctx, |ui| {
                    let (rect, response) = ui
                        .allocate_exact_size(egui::Vec2::splat(BADGE * 2.4), egui::Sense::click());
                    let p = ui.painter();
                    if badge.closed {
                        p.circle_filled(rect.center(), BADGE, tokens::SUCCESS);
                    } else {
                        p.circle_filled(rect.center(), BADGE, tokens::BG_1);
                        p.circle_stroke(
                            rect.center(),
                            BADGE,
                            egui::Stroke::new(1.5, tokens::TEXT_MUTED),
                        );
                    }
                    let name = names.get(&badge.symbol).map_or("", String::as_str);
                    let state = if badge.closed { "closed" } else { "open" };
                    let tip = if driven.contains(&badge.symbol) {
                        format!("{name} · {state}, follows its coil")
                    } else {
                        format!(
                            "{name} · {state} · click to {}, Shift+click for every pole",
                            if badge.closed { "open" } else { "close" },
                        )
                    };
                    if response.on_hover_text(tip).clicked() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        clicked = Some((badge.symbol, badge.group, shift));
                    }
                });
        }

        // Coil badges: a click switches the coil on or off by hand.
        let mut coil_clicked: Option<SymbolId> = None;
        for badge in &coils {
            let at = to_screen(badge.at);
            if !canvas.expand(BADGE).contains(at) {
                continue;
            }
            let id = egui::Id::new(("live_coil", badge.symbol));
            egui::Area::new(id)
                .order(egui::Order::Middle)
                .pivot(egui::Align2::CENTER_CENTER)
                .fixed_pos(at)
                .show(ctx, |ui| {
                    let (rect, response) = ui
                        .allocate_exact_size(egui::Vec2::splat(BADGE * 2.4), egui::Sense::click());
                    let square =
                        egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(BADGE * 2.0));
                    let p = ui.painter();
                    if badge.on {
                        p.rect_filled(square, 2.0, tokens::SUCCESS);
                    } else {
                        p.rect_filled(square, 2.0, tokens::BG_1);
                        p.rect_stroke(
                            square,
                            2.0,
                            egui::Stroke::new(1.5, tokens::TEXT_MUTED),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let name = symbol_name(project, badge.symbol);
                    let tip = format!(
                        "{name} · coil {} · click to switch it {}",
                        if badge.on { "on" } else { "off" },
                        if badge.on { "off" } else { "on" },
                    );
                    if response.on_hover_text(tip).clicked() {
                        coil_clicked = Some(badge.symbol);
                    }
                });
        }

        let closed = badges.iter().filter(|b| b.closed).count();
        let mut open_all = false;
        let mut leave = false;
        egui::Area::new(egui::Id::new("live_card"))
            .order(egui::Order::Middle)
            .fixed_pos(canvas.left_top() + egui::vec2(12.0, 10.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(240.0);
                    ui.horizontal(|ui| {
                        widgets::section_label(ui, "Live potentials");
                        if ui
                            .small_button("✕")
                            .on_hover_text("Close · Shift+L")
                            .clicked()
                        {
                            leave = true;
                        }
                    });
                    if badges.is_empty() {
                        ui.weak("No switched contacts on this page");
                    } else {
                        ui.horizontal(|ui| {
                            ui.weak(format!("{closed} of {} poles closed", badges.len()));
                            if ui
                                .add_enabled(closed > 0, egui::Button::new("All open").small())
                                .clicked()
                            {
                                open_all = true;
                            }
                        });
                    }
                    ui.add_space(2.0);
                    for (potential, nets) in &legend.carried {
                        let Some((name, color)) = colors.get(potential) else {
                            continue;
                        };
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(14.0, 4.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, *color);
                            ui.label(name);
                            ui.weak(format!("{nets} net{}", if *nets == 1 { "" } else { "s" }));
                        });
                    }
                    if legend.shorted > 0 {
                        ui.colored_label(
                            tokens::ERROR,
                            format!(
                                "{} short{}: potentials meet",
                                legend.shorted,
                                if legend.shorted == 1 { "" } else { "s" }
                            ),
                        );
                    }
                    if let Some(left) = timing {
                        ui.weak(format!("A timer is running, {left:.1} s left"));
                    }
                    if legend.carried.is_empty() && legend.shorted == 0 {
                        ui.weak("Nothing is energized");
                    }
                });
            });

        // Draws again only when something changed; egui repaints on input.
        if leave || open_all || clicked.is_some() || timing.is_some() || animating {
            ctx.request_repaint();
        }
        if leave {
            self.wb.live.on = false;
        }
        if open_all {
            self.wb.live.poles = PoleState::all_open();
            self.wb.live.forced.clear();
            self.wb.live.clock.reset();
        }
        if let Some(coil) = coil_clicked
            && !self.wb.live.forced.remove(&coil)
        {
            self.wb.live.forced.insert(coil);
        }
        if let Some((symbol, group, all)) = clicked
            && !driven.contains(&symbol)
        {
            if all {
                if let Some(doc) = self.backend.document() {
                    self.wb.live.flip_all(doc.project(), symbol);
                }
            } else {
                self.wb.live.flip(symbol, group);
            }
        }
    }
}

/// A symbol's name for a tooltip: its device tag, else its library name.
fn symbol_name(project: &Project, symbol: SymbolId) -> String {
    let Some(s) = project.symbol(symbol) else {
        return String::new();
    };
    match &s.role {
        SymbolRole::Device { device, .. } => project
            .device(*device)
            .map_or_else(|| s.symbol.0.clone(), |d| d.tag.to_string()),
        _ => s.symbol.0.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{Document, Endpoint, Pin, Placement, Point};

    fn block(tx: &mut novo_core::Tx<'_>, page: PageId, y: f64, role: SymbolRole) -> SymbolId {
        tx.place_symbol(
            page,
            "test/block",
            Placement::at(Point::new(0.0, y)),
            vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
            role,
        )
        .unwrap()
    }

    /// L1 feed, a contact, a load. Returns the page and the contact.
    fn circuit(doc: &mut Document) -> (PageId, SymbolId) {
        let (out, _) = doc
            .transact("Draw", |tx| {
                let page = tx.add_page("1")?;
                let l1 = tx.add_potential("L1", PotentialKind::Line)?;
                let feed = block(tx, page, 0.0, SymbolRole::PotentialFeed { potential: l1 });
                let contact = block(tx, page, 40.0, SymbolRole::Unassigned);
                tx.update::<novo_core::SymbolInstance>(contact, |s| {
                    s.pass_through = vec![vec![0, 1]]
                })?;
                let load = block(tx, page, 80.0, SymbolRole::Unassigned);
                tx.connect(Endpoint::pin(feed, 1), Endpoint::pin(contact, 0))?;
                tx.connect(Endpoint::pin(contact, 1), Endpoint::pin(load, 0))?;
                Ok((page, contact))
            })
            .unwrap();
        out
    }

    #[test]
    fn closing_a_pole_lights_the_wire_behind_it() {
        let mut doc = Document::default();
        let (page, contact) = circuit(&mut doc);
        let mut live = LiveView::default();
        let carrying = |live: &mut LiveView| {
            let solved = live.solved(doc.project(), 0.0);
            lit_wires(
                doc.project(),
                page,
                &solved.netlist,
                &solved.settled.solution,
            )
            .iter()
            .filter(|w| matches!(w.live, Live::Carries(_)))
            .count()
        };
        // The wire from the feed to the contact is lit at rest; the one
        // behind the contact is not.
        assert_eq!(carrying(&mut live), 1);
        live.flip(contact, 0);
        assert_eq!(carrying(&mut live), 2);
        live.flip(contact, 0);
        assert_eq!(carrying(&mut live), 1);
        live.flip_all(doc.project(), contact);
        assert!(live.poles.is_closed(contact, 0));
    }

    #[test]
    fn the_legend_counts_nets_and_each_pole_gets_a_badge() {
        let mut doc = Document::default();
        let (page, contact) = circuit(&mut doc);
        let mut live = LiveView::default();
        live.flip(contact, 0);
        let solved = live.solved(doc.project(), 0.0);
        let wires = lit_wires(
            doc.project(),
            page,
            &solved.netlist,
            &solved.settled.solution,
        );
        let l = legend(
            &wires,
            doc.project(),
            page,
            &solved.netlist,
            &solved.settled.solution,
        );
        assert_eq!(l.carried.len(), 1);
        assert_eq!(*l.carried.values().next().unwrap(), 2);
        assert_eq!(l.shorted, 0);
        let badges = pole_badges(doc.project(), page, &live.poles);
        assert_eq!(badges.len(), 1);
        assert!(badges[0].closed);
        assert_eq!(badges[0].at, egui::vec2(0.0, 44.0));
    }

    #[test]
    fn line_conductors_get_separate_colours() {
        let mut doc = Document::default();
        doc.transact("Potentials", |tx| {
            for name in ["L1", "L2", "L3"] {
                tx.add_potential(name, PotentialKind::Line)?;
            }
            Ok(())
        })
        .unwrap();
        let colors: Vec<_> = doc.project().potentials().map(potential_color).collect();
        assert_eq!(colors.len(), 3);
        assert!(colors[0] != colors[1] && colors[1] != colors[2] && colors[0] != colors[2]);
    }
}

#[cfg(test)]
mod hover_tests {
    use super::*;

    #[test]
    fn distance_to_a_segment_clamps_at_its_ends() {
        let a = egui::pos2(0.0, 0.0);
        let b = egui::pos2(10.0, 0.0);
        assert_eq!(distance_to_segment(egui::pos2(5.0, 3.0), a, b), 3.0);
        assert_eq!(distance_to_segment(egui::pos2(-4.0, 3.0), a, b), 5.0);
        assert_eq!(distance_to_segment(egui::pos2(1.0, 1.0), a, a), 2f32.sqrt());
    }
}
