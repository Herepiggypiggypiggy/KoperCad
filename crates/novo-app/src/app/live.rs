//! The live potentials view (point 22, display side): Shift+L turns the
//! drawing into a bench where every switched pole can be closed or opened
//! and the wires show what they carry under that state, as solved by
//! novo-sim. Nothing here touches the model: the states live in the
//! workbench and the wires are lit in the canvas's own wire layer.
//!
//! Poles start as a plant at rest (break contacts closed, breakers on). A badge sits on every switched pole of
//! the page: hold a pushbutton to press it, click any other pole to close or open every pole of the device together,
//! Shift+click to drive that one pole apart (its badge then gets an amber ring). A wire carrying one potential is lit in that
//! potential's colour, wires nothing reaches are dimmed, and a net where two
//! potentials meet is drawn red.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use eframe::egui;
use novo_canvas::model::WireLook;
use novo_core::{
    Endpoint, NetId, Netlist, PageId, PinRef, Potential, PotentialId, PotentialKind, Project,
    SymbolId, SymbolRole, WireId,
};
use novo_sim::{
    Clock, Live, PoleState, Settled, Solution, inputs_seen, is_coil, is_lamp, is_load, is_motor,
    is_plc_input, is_plc_output, loads_on, settle_timed,
};
use novo_theme::{tokens, widgets};

use super::live_anim::{Anim, Frame};
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
    /// A pushbutton held down: the symbol and the poles as they were before.
    held: Option<(SymbolId, PoleState)>,
    /// Coils switched on by hand, whatever feeds them.
    pub(super) forced: BTreeSet<SymbolId>,
    /// Seconds the bench has run, for timer contacts.
    clock: Clock,
    /// Easing state of the device animation (lamps, motors, coils).
    anim: Anim,
    solved: Option<Solved>,
    /// The PLC program window and the program running in it.
    pub(super) plc: super::plc_run::PlcRun,
}

/// A solution for one project revision and one pole state.
struct Solved {
    revision: u64,
    poles: PoleState,
    forced: BTreeSet<SymbolId>,
    netlist: Netlist,
    settled: Settled,
    /// Output channels a running PLC program has on.
    plc_on: BTreeSet<SymbolId>,
}

impl LiveView {
    /// The solution for `project` under the current poles, solved again only
    /// when the drawing or the poles changed.
    fn solved(&mut self, project: &Project, dt: f64) -> &Solved {
        if self.plc.is_running() {
            // A program is scanning: every frame is a scan, and the circuit
            // is solved again with the outputs it just set.
            let netlist = match self.solved.take() {
                Some(s) if s.revision == project.revision() && s.netlist.is_current(project) => {
                    s.netlist
                }
                _ => project.netlist(),
            };
            if let Some(settled) =
                self.plc
                    .step(project, &netlist, &self.poles, &self.forced, dt.min(0.1))
            {
                self.solved = Some(Solved {
                    revision: project.revision(),
                    poles: self.poles.clone(),
                    forced: self.forced.clone(),
                    netlist,
                    settled,
                    plc_on: self.plc.outputs_on(),
                });
                return self.solved.as_ref().expect("set above");
            }
        }
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
                plc_on: BTreeSet::new(),
            });
        }
        self.solved.as_ref().expect("solved above")
    }

    /// Forgets the last solution: the next frame solves again.
    pub(super) fn invalidate(&mut self) {
        self.solved = None;
    }

    /// The clock timer contacts count on: the program's while it runs.
    pub(super) fn timer_clock(&self) -> &Clock {
        self.plc.clock().unwrap_or(&self.clock)
    }

    /// What a lesson step can read off the live view: the devices whose coil
    /// is energized, the pins that carry a potential and the devices with a
    /// pole closed, all by tag and designation.
    pub(super) fn learn_facts(
        &mut self,
        project: &Project,
    ) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
        let tag_of = |symbol: SymbolId| {
            project
                .pin_designation(PinRef::new(symbol, 0))
                .and_then(|d| d.rsplit_once(':').map(|(tag, _)| tag.to_owned()))
        };
        let solved = self.solved(project, 0.0);
        let coils = solved
            .settled
            .energized
            .iter()
            .filter_map(|s| tag_of(*s))
            .collect();
        let closed = project
            .symbols()
            .filter(|s| (0..s.pass_through.len()).any(|g| solved.settled.poles.is_closed(s.id, g)))
            .filter_map(|s| tag_of(s.id))
            .collect();
        let mut pins = BTreeSet::new();
        for symbol in project.symbols() {
            for index in 0..symbol.pins.len() {
                let Ok(pin) = u16::try_from(index) else {
                    continue;
                };
                let live = solved.settled.solution.of(
                    &solved.netlist,
                    Endpoint::Pin {
                        symbol: symbol.id,
                        pin,
                    },
                );
                if !matches!(live, Live::Dead)
                    && let Some(d) = project.pin_designation(PinRef::new(symbol.id, pin))
                {
                    pins.insert(d);
                }
            }
        }
        (coils, pins, closed)
    }

    /// Closes the pole if open and opens it if closed.
    pub(super) fn flip(&mut self, symbol: SymbolId, group: usize) {
        let closed = self.poles.is_closed(symbol, group);
        self.poles.set(symbol, group, !closed);
    }

    /// Switches every pole of a symbol (one device, poles joined by its
    /// mechanical link) to the opposite of the pole that was clicked. After a
    /// per-pole override this puts the poles back in step.
    pub(super) fn flip_together(&mut self, project: &Project, symbol: SymbolId, group: usize) {
        let closed = self.poles.is_closed(symbol, group);
        for member in together(project, symbol) {
            self.poles.set_all(project, member, !closed);
        }
    }

    /// Puts every pole in the state of a plant at rest.
    pub(super) fn reset_poles(&mut self, project: &Project) {
        self.poles = novo_sim::rest_state(project);
        self.held = None;
    }

    /// A pushbutton goes down: its poles switch to the opposite of the
    /// pressed pole until [`release`](Self::release).
    fn press(&mut self, project: &Project, symbol: SymbolId, group: usize) {
        if self.held.is_none() {
            self.held = Some((symbol, self.poles.clone()));
            self.flip_together(project, symbol, group);
        }
    }

    /// The pushbutton comes back up and its poles with it.
    fn release(&mut self, project: &Project) {
        if let Some((symbol, before)) = self.held.take() {
            for member in together(project, symbol) {
                for group in 0..project.symbol(member).map_or(0, |s| s.pass_through.len()) {
                    self.poles
                        .set(member, group, before.is_closed(member, group));
                }
            }
        }
    }
}

/// The symbols that switch together with `symbol`: the ones drawn for the
/// same device from the same library symbol (the three single-pole halves of
/// a breaker, joined by its mechanical link), `symbol` itself first.
pub(super) fn together(project: &Project, symbol: SymbolId) -> Vec<SymbolId> {
    let Some(this) = project.symbol(symbol) else {
        return vec![symbol];
    };
    let mut members = vec![symbol];
    if let Some(device) = this.device() {
        members.extend(
            project
                .symbols_of_device(device)
                .into_iter()
                .filter(|s| s.id != symbol && s.symbol == this.symbol && !s.pass_through.is_empty())
                .map(|s| s.id),
        );
    }
    members
}

/// A pushbutton or foot switch: it springs back when let go.
fn is_momentary(project: &Project, symbol: SymbolId) -> bool {
    project.symbol(symbol).is_some_and(|s| {
        let name = s.symbol.0.rsplit('/').next().unwrap_or("");
        s.symbol.0.contains("/operators/")
            && (name.starts_with("pushbutton") || name.starts_with("footswitch"))
    })
}

/// One wire of the page and what it carries.
pub(super) struct LitWire {
    pub id: WireId,
    pub net: Option<NetId>,
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
            let net = netlist.net_of_wire(w);
            let live = net.map_or(Live::Dead, |n| solution.net(n).clone());
            Some(LitWire {
                id: w.id,
                net,
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
    /// The poles of this symbol are not all in the same state (a per-pole override).
    pub mixed: bool,
}

pub(super) fn pole_badges(project: &Project, page: PageId, poles: &PoleState) -> Vec<PoleBadge> {
    let mut badges = Vec::new();
    for symbol in project.symbols().filter(|s| s.page == page) {
        let (mut closed_poles, mut all_poles) = (0, 0);
        for member in together(project, symbol.id) {
            let n = project.symbol(member).map_or(0, |m| m.pass_through.len());
            all_poles += n;
            closed_poles += (0..n).filter(|&g| poles.is_closed(member, g)).count();
        }
        let mixed = closed_poles != 0 && closed_poles != all_poles;
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
                mixed,
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
    /// A PLC digital output rather than a coil.
    pub output: bool,
}

/// The coils of the page and the PLC outputs, which are switched by hand
/// just the same.
pub(super) fn coil_badges(
    project: &Project,
    page: PageId,
    energized: &BTreeSet<SymbolId>,
    forced: &BTreeSet<SymbolId>,
) -> Vec<CoilBadge> {
    project
        .symbols()
        .filter(|s| s.page == page)
        .filter_map(|s| {
            if is_coil(s) {
                let a = s.pin_position(0)?;
                let b = s.pin_position(1)?;
                Some(CoilBadge {
                    symbol: s.id,
                    at: egui::vec2(((a.x + b.x) / 2.0) as f32, ((a.y + b.y) / 2.0) as f32),
                    on: energized.contains(&s.id),
                    output: false,
                })
            } else if is_plc_output(s) {
                let a = s.pin_position(0)?;
                Some(CoilBadge {
                    symbol: s.id,
                    at: egui::vec2(a.x as f32, (a.y - 5.0) as f32),
                    on: forced.contains(&s.id),
                    output: true,
                })
            } else {
                None
            }
        })
        .collect()
}

/// The PLC inputs of the page with their pin positions, and whether the pin
/// carries a potential.
pub(super) fn input_marks(
    project: &Project,
    page: PageId,
    seen: &BTreeSet<SymbolId>,
) -> Vec<(egui::Vec2, bool)> {
    project
        .symbols()
        .filter(|s| s.page == page && is_plc_input(s))
        .filter_map(|s| {
            let a = s.pin_position(0)?;
            Some((egui::vec2(a.x as f32, a.y as f32), seen.contains(&s.id)))
        })
        .collect()
}

/// Where the lit horns, buzzers and valves of the page are, for a glow.
pub(super) fn load_glows(
    project: &Project,
    page: PageId,
    netlist: &Netlist,
    solution: &Solution,
) -> Vec<(SymbolId, egui::Vec2)> {
    let on = loads_on(project, netlist, solution);
    project
        .symbols()
        .filter(|s| s.page == page && is_load(s) && on.contains(&s.id))
        // Lamps and motors are animated by live_anim; the rest still glow.
        .filter(|s| !is_lamp(s) && !is_motor(s))
        .filter_map(|s| {
            let points: Vec<_> = (0..s.pins.len())
                .filter_map(|pin| s.pin_position(pin as u16))
                .collect();
            if points.is_empty() {
                return None;
            }
            let n = points.len() as f64;
            let x = points.iter().map(|p| p.x).sum::<f64>() / n;
            let y = points.iter().map(|p| p.y).sum::<f64>() / n;
            Some((s.id, egui::vec2(x as f32, y as f32)))
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
            if let Some(doc) = self.backend.document() {
                self.wb.live.reset_poles(doc.project());
            }
            self.tour_tick("live");
            self.message(
                Level::Info,
                "Live potentials: the plant is at rest, break contacts closed. Click a pole badge to switch the device, hold a pushbutton",
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
        live.plc.sync(project, view.editor.catalog.library());
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
        let shown: BTreeSet<SymbolId> = solved.forced.union(&solved.plc_on).copied().collect();
        let coils = coil_badges(project, page, &energized, &shown);
        let seen = inputs_seen(project, &solved.netlist, &solved.settled.solution);
        let inputs = input_marks(project, page, &seen);
        let colors: BTreeMap<PotentialId, (String, egui::Color32)> = project
            .potentials()
            .map(|p| (p.id, (p.name.clone(), potential_color(p))))
            .collect();
        let states = novo_sim::device_states(project, &solved.netlist, &solved.settled);
        let poles = solved.settled.poles.clone();
        let analysis = self
            .wb
            .sim
            .analysis(project, &solved.netlist, &solved.settled.poles)
            .clone();
        let glows = load_glows(project, page, &solved.netlist, &solved.settled.solution);
        let timers = novo_sim::timer_progress(project, live.timer_clock());
        let names: BTreeMap<SymbolId, String> = badges
            .iter()
            .map(|b| (b.symbol, symbol_name(project, b.symbol)))
            .collect();

        // Devices: lamps glow, motors turn, coils pull in.
        let animating = self.wb.live.anim.paint(&Frame {
            ctx,
            canvas,
            zoom: viewport.zoom,
            to_screen: &to_screen,
            project,
            library: self
                .canvas
                .schematic_ref()
                .map(|v| v.editor.catalog.library()),
            page,
            states: &states,
            energized: &energized,
            poles: &poles,
            timers: &timers,
            reduced: self.settings.reduce_motion,
            dt: dt as f32,
        });

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
                let text = match wire.net {
                    Some(net) if !matches!(wire.live, Live::Shorted(_)) => {
                        format!(
                            "{text} · {}",
                            super::simulate::reading_text(analysis.net(net))
                        )
                    }
                    _ => text,
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
        let mut pressed: Option<(SymbolId, usize)> = None;
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
                    if badge.mixed {
                        p.circle_stroke(
                            rect.center(),
                            BADGE + 2.0,
                            egui::Stroke::new(1.5, tokens::WARNING),
                        );
                    }
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
                    let state = match (badge.closed, badge.mixed) {
                        (true, true) => "closed, driven apart from its other poles",
                        (false, true) => "open, driven apart from its other poles",
                        (true, false) => "closed",
                        (false, false) => "open",
                    };
                    let tip = if driven.contains(&badge.symbol) {
                        format!("{name} · {state}, follows its coil")
                    } else {
                        format!(
                            "{name} · {state} · click to {} every pole, Shift+click for this pole only",
                            if badge.closed { "open" } else { "close" },
                        )
                    };
                    let shift = ui.input(|i| i.modifiers.shift);
                    let momentary = is_momentary(project, badge.symbol) && !shift;
                    if momentary && response.is_pointer_button_down_on() {
                        pressed = Some((badge.symbol, badge.group));
                    }
                    if response.on_hover_text(tip).clicked() && !momentary {
                        clicked = Some((badge.symbol, badge.group, !shift));
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
                        "{name} · {} {} · click to switch it {}",
                        if badge.output { "output" } else { "coil" },
                        if badge.on { "on" } else { "off" },
                        if badge.on { "off" } else { "on" },
                    );
                    if response.on_hover_text(tip).clicked() {
                        coil_clicked = Some(badge.symbol);
                    }
                });
        }

        // PLC inputs: a dot that fills when the pin carries a potential.
        for (n, (at, high)) in inputs.iter().enumerate() {
            let at = to_screen(*at);
            if !canvas.expand(BADGE).contains(at) {
                continue;
            }
            egui::Area::new(egui::Id::new(("live_input", n)))
                .order(egui::Order::Middle)
                .pivot(egui::Align2::CENTER_CENTER)
                .fixed_pos(at)
                .interactable(false)
                .show(ctx, |ui| {
                    let (rect, _) = ui
                        .allocate_exact_size(egui::Vec2::splat(BADGE * 1.6), egui::Sense::hover());
                    let p = ui.painter();
                    if *high {
                        p.circle_filled(rect.center(), BADGE * 0.7, tokens::SUCCESS);
                    } else {
                        p.circle_stroke(
                            rect.center(),
                            BADGE * 0.7,
                            egui::Stroke::new(1.5, tokens::TEXT_MUTED),
                        );
                    }
                });
        }

        // Outputs that have current: a soft glow behind the lamp or motor.
        for (symbol, at) in &glows {
            let at = to_screen(*at);
            if !canvas.expand(BADGE * 4.0).contains(at) {
                continue;
            }
            egui::Area::new(egui::Id::new(("live_load", *symbol)))
                .order(egui::Order::Background)
                .pivot(egui::Align2::CENTER_CENTER)
                .fixed_pos(at)
                .interactable(false)
                .show(ctx, |ui| {
                    let (rect, _) = ui
                        .allocate_exact_size(egui::Vec2::splat(BADGE * 5.0), egui::Sense::hover());
                    let p = ui.painter();
                    p.circle_filled(
                        rect.center(),
                        BADGE * 2.5,
                        tokens::WARNING.gamma_multiply(0.18),
                    );
                    p.circle_filled(
                        rect.center(),
                        BADGE * 1.5,
                        tokens::WARNING.gamma_multiply(0.35),
                    );
                });
        }

        let closed = badges.iter().filter(|b| b.closed).count();
        let mut open_all = false;
        let mut leave = false;
        let mut open_plc = false;
        let plc_running = self.wb.live.plc.is_running();
        let mut open_motor: Option<SymbolId> = None;
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
                                .small_button("Reset")
                                .on_hover_text("Back to the plant at rest")
                                .clicked()
                            {
                                open_all = true;
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .small_button("PLC program…")
                            .on_hover_text(
                                "Run a ladder or Structured Text program on the drawn I/O",
                            )
                            .clicked()
                        {
                            open_plc = true;
                        }
                        if plc_running {
                            ui.colored_label(tokens::SUCCESS, "running");
                        }
                    });
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
                    open_motor = super::simulate::values_block(ui, project, &analysis, &|s| {
                        symbol_name(project, s)
                    });
                    if legend.carried.is_empty() && legend.shorted == 0 {
                        ui.weak("Nothing is energized");
                    }
                });
            });

        // Draws again only when something changed; egui repaints on input.
        if open_plc {
            self.wb.live.plc.window = true;
        }
        if leave || open_all || clicked.is_some() || timing.is_some() || animating || plc_running {
            ctx.request_repaint();
        }
        if let Some(motor) = open_motor {
            self.open_motor_start_for(motor);
        }
        if leave {
            self.wb.live.on = false;
        }
        if open_all {
            if let Some(doc) = self.backend.document() {
                self.wb.live.reset_poles(doc.project());
            }
            self.wb.live.forced.clear();
            self.wb.live.clock.reset();
        }
        if let Some(coil) = coil_clicked
            && !self.wb.live.forced.remove(&coil)
        {
            self.wb.live.forced.insert(coil);
        }
        match pressed {
            Some((symbol, group)) if !driven.contains(&symbol) => {
                if let Some(doc) = self.backend.document() {
                    self.wb.live.press(doc.project(), symbol, group);
                }
                ctx.request_repaint();
            }
            None if self.wb.live.held.is_some() => {
                if let Some(doc) = self.backend.document() {
                    self.wb.live.release(doc.project());
                }
                ctx.request_repaint();
            }
            _ => {}
        }
        if let Some((symbol, group, together)) = clicked
            && !driven.contains(&symbol)
        {
            if together {
                if let Some(doc) = self.backend.document() {
                    self.wb.live.flip_together(doc.project(), symbol, group);
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
        live.flip_together(doc.project(), contact, 0);
        assert!(live.poles.is_closed(contact, 0));
    }

    fn three_pole(doc: &mut Document) -> SymbolId {
        let (id, _) = doc
            .transact("Breaker", |tx| {
                let page = tx.add_page("1")?;
                let q = block(tx, page, 0.0, SymbolRole::Unassigned);
                tx.update::<novo_core::SymbolInstance>(q, |s| {
                    s.pass_through = vec![vec![0, 1]; 3]
                })?;
                Ok(q)
            })
            .unwrap();
        id
    }

    #[test]
    fn a_click_switches_all_poles_of_a_breaker_together() {
        let mut doc = Document::default();
        let q = three_pole(&mut doc);
        let mut live = LiveView::default();
        live.flip_together(doc.project(), q, 1);
        assert!((0..3).all(|g| live.poles.is_closed(q, g)));
        live.flip_together(doc.project(), q, 2);
        assert!((0..3).all(|g| !live.poles.is_closed(q, g)));
    }

    #[test]
    fn a_pole_override_moves_one_pole_and_the_next_click_puts_them_back_in_step() {
        let mut doc = Document::default();
        let q = three_pole(&mut doc);
        let page = doc.project().symbol(q).unwrap().page;
        let mut live = LiveView::default();
        live.flip_together(doc.project(), q, 0);
        live.flip(q, 1);
        assert!(live.poles.is_closed(q, 0) && !live.poles.is_closed(q, 1));
        let badges = pole_badges(doc.project(), page, &live.poles);
        assert!(badges.iter().all(|b| b.mixed));
        // Clicking the open pole closes every pole; clicking a closed one opens them.
        live.flip_together(doc.project(), q, 1);
        assert!((0..3).all(|g| live.poles.is_closed(q, g)));
        let badges = pole_badges(doc.project(), page, &live.poles);
        assert!(badges.iter().all(|b| !b.mixed));
        live.flip(q, 2);
        live.flip_together(doc.project(), q, 0);
        assert!((0..3).all(|g| !live.poles.is_closed(q, g)));
    }

    #[test]
    fn single_pole_symbols_of_one_device_switch_together_and_a_pushbutton_springs_back() {
        let mut doc = Document::default();
        let (page, poles) = doc
            .transact("Breaker", |tx| {
                let page = tx.add_page("1")?;
                let f1 = tx.add_device(novo_core::DeviceTag::product("F1"), "")?;
                let s1 = tx.add_device(novo_core::DeviceTag::product("S1"), "")?;
                let mut ids = Vec::new();
                for _ in 0..3 {
                    let id = block(
                        tx,
                        page,
                        0.0,
                        SymbolRole::Device {
                            device: f1,
                            main: true,
                        },
                    );
                    tx.update::<novo_core::SymbolInstance>(id, |s| {
                        s.symbol = novo_core::SymbolKey("iec/protection/mcb_1p".into());
                        s.pass_through = vec![vec![0, 1]]
                    })?;
                    ids.push(id);
                }
                let push = block(
                    tx,
                    page,
                    0.0,
                    SymbolRole::Device {
                        device: s1,
                        main: true,
                    },
                );
                tx.update::<novo_core::SymbolInstance>(push, |s| {
                    s.symbol = novo_core::SymbolKey("iec/operators/pushbutton_no".into());
                    s.pass_through = vec![vec![0, 1]]
                })?;
                ids.push(push);
                Ok((page, ids))
            })
            .unwrap()
            .0;
        let project = doc.project();
        let mut live = LiveView::default();
        live.reset_poles(project);
        // Breaker halves start on and switch off together from any one of them.
        assert!(poles[..3].iter().all(|&p| live.poles.is_closed(p, 0)));
        live.flip_together(project, poles[1], 0);
        assert!(poles[..3].iter().all(|&p| !live.poles.is_closed(p, 0)));
        live.flip(poles[2], 0);
        assert!(
            pole_badges(project, page, &live.poles)
                .iter()
                .filter(|b| poles[..3].contains(&b.symbol))
                .all(|b| b.mixed)
        );
        // A pushbutton is closed only while held.
        let push = poles[3];
        assert!(is_momentary(project, push) && !is_momentary(project, poles[0]));
        assert!(!live.poles.is_closed(push, 0));
        live.press(project, push, 0);
        assert!(live.poles.is_closed(push, 0));
        live.release(project);
        assert!(!live.poles.is_closed(push, 0));
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
