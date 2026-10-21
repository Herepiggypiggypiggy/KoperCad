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

use super::live_anim::{Anim, Frame};
use super::{Level, NovoApp};

/// Radius of a pole badge, in screen points.
const BADGE: f32 = 6.0;
/// How close the pointer must be to a wire to describe it, in screen points.
const HOVER_REACH: f32 = 5.0;
/// How far beyond a device's pins a click still asks about the device, in
/// screen points.
const WHY_REACH: f32 = 10.0;
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
    /// Click mode of "Why?": the next click on a wire or device asks why it
    /// is lit or dead (Alt+click does the same without the mode).
    pub(super) why_mode: bool,
    /// The frame whose Escape was handled, so a repeated pass of one frame
    /// does not step back twice.
    escaped_in: Option<u64>,
    /// What the explanation is about.
    why: Option<Target>,
    /// The PLC program window and the program running in it.
    pub(super) plc: super::plc_run::PlcRun,
}

/// What "Why?" was asked about. A wire is named by its id and a device by its
/// symbol, since net numbers change whenever the netlist is rebuilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Wire(WireId),
    Symbol(SymbolId),
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

    /// Switches the live view off and drops what it was asking.
    pub(super) fn leave(&mut self) {
        self.on = false;
        self.why_mode = false;
        self.why = None;
        self.held = None;
    }

    /// Escape steps back one level: the Why? click mode, then its answer,
    /// then the live view itself.
    pub(super) fn escape(&mut self) {
        if self.why_mode {
            self.why_mode = false;
        } else if self.why.is_some() {
            self.why = None;
        } else {
            self.leave();
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
pub(super) fn is_momentary(project: &Project, symbol: SymbolId) -> bool {
    project
        .symbol(symbol)
        .is_some_and(|s| novo_sim::explain::is_pushbutton_key(&s.symbol.0))
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

/// A hand-operated device for the Operate list.
pub(super) struct Operator {
    /// The first symbol of the device; the others follow it.
    pub symbol: SymbolId,
    pub label: String,
    pub kind: &'static str,
    /// What the state is in words.
    pub state: &'static str,
    /// What the button does when used.
    pub action: &'static str,
    pub momentary: bool,
    /// Away from its rest position.
    pub active: bool,
}

/// The push buttons, selectors, emergency stops and breakers of the page that
/// no coil drives, one row per device, in tag order.
pub(super) fn operators(project: &Project, page: PageId, poles: &PoleState) -> Vec<Operator> {
    use novo_sim::explain::{HandKind, hand_kind};
    let rest = novo_sim::rest_state(project);
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for s in project.symbols().filter(|s| s.page == page) {
        let key = s.symbol.0.as_str();
        if s.pass_through.is_empty()
            || s.device().is_none()
            || !(key.contains("/operators/") || hand_kind(key) == HandKind::Protective)
        {
            continue;
        }
        // A contact a coil drives is not operated by hand.
        if s.device().is_some_and(|d| {
            project
                .symbols_of_device(d)
                .iter()
                .any(|m| novo_sim::is_coil(m))
        }) {
            continue;
        }
        let first = together(project, s.id)[0];
        if s.device().is_some_and(|d| !seen.insert(d)) {
            continue;
        }
        let active = together(project, s.id).iter().any(|&m| {
            (0..project.symbol(m).map_or(0, |x| x.pass_through.len()))
                .any(|g| poles.is_closed(m, g) != rest.is_closed(m, g))
        });
        let kind = hand_kind(key);
        let (kind_text, state, action) = match (kind, active) {
            (HandKind::Pushbutton, false) => ("push button", "released", "Hold to press"),
            (HandKind::Pushbutton, true) => ("push button", "pressed", "Hold to press"),
            (HandKind::EmergencyStop, false) => ("emergency stop", "released", "Press in"),
            (HandKind::EmergencyStop, true) => ("emergency stop", "pressed in", "Release"),
            (HandKind::Protective, false) => ("breaker", "on", "Switch off"),
            (HandKind::Protective, true) => ("breaker", "off", "Switch on"),
            (HandKind::Switch, false) => ("switch", "at rest", "Operate"),
            (HandKind::Switch, true) => ("switch", "operated", "Return"),
        };
        out.push(Operator {
            symbol: first,
            label: symbol_name(project, s.id),
            kind: kind_text,
            state,
            action,
            momentary: kind == HandKind::Pushbutton,
            active,
        });
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
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

    /// `sim.why`: switches the live view on if it is off and waits for a
    /// click on a wire or a device.
    pub(super) fn ask_why(&mut self) {
        if !self.wb.live.on {
            self.toggle_live();
        }
        self.wb.live.why_mode = true;
        self.message(
            Level::Info,
            "Click a wire or a device to see why it is lit or dead",
        );
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
        // Escape, when nothing else is using it (the command field, the
        // pages flyout, a menu or a text box).
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && !self.wb.field.is_open()
            && !self.outline_open
            && !ctx.egui_wants_keyboard_input()
            && !egui::Popup::is_any_open(ctx)
            && self.wb.live.escaped_in != Some(ctx.cumulative_frame_nr())
        {
            self.wb.live.escaped_in = Some(ctx.cumulative_frame_nr());
            self.wb.live.escape();
        }
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
        let why_target = live.why;
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
        let story = novo_sim::explain::story(project, &solved.settled, &states, &|s| {
            symbol_name(project, s)
        });
        let answer = why_target.map(|target| {
            let name = |s: SymbolId| symbol_name(project, s);
            match target {
                Target::Wire(id) => {
                    wires
                        .iter()
                        .find(|w| w.id == id)
                        .and_then(|w| w.net)
                        .map(|net| {
                            novo_sim::explain::answer_net(
                                project,
                                &solved.netlist,
                                &solved.settled,
                                net,
                                &name,
                            )
                        })
                }
                Target::Symbol(id) => Some(novo_sim::explain::answer_symbol(
                    project,
                    &solved.netlist,
                    &solved.settled,
                    &solved.forced,
                    id,
                    &name,
                )),
            }
        });
        let answer = answer.flatten();
        let path_nets: BTreeSet<NetId> = answer
            .as_ref()
            .map(|a| a.nets.iter().copied().collect())
            .unwrap_or_default();
        let break_symbols: BTreeSet<SymbolId> = answer
            .as_ref()
            .map(|a| a.breaks.iter().copied().collect())
            .unwrap_or_default();
        let answer_dead = answer.as_ref().is_some_and(|a| a.dead);
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
        let dead_wire = novo_theme::POTENTIALS_DARK.dead;
        let looks = wires
            .iter()
            .map(|wire| {
                let (color, width) = match &wire.live {
                    Live::Dead => (novo_theme::canvas::to_color32(dead_wire), 2.5),
                    Live::Carries(p) => {
                        let color = colors
                            .get(p)
                            .map_or(crate::theme::ui_palette().accent, |c| c.1);
                        (color.gamma_multiply(0.9), LIT_WIDTH)
                    }
                    Live::Shorted(_) => (crate::theme::ui_palette().error, LIT_WIDTH + 1.0),
                };
                // The route of a "Why?" answer is drawn heavier, in amber when
                // it ends in a break.
                let on_path = wire.net.is_some_and(|n| path_nets.contains(&n));
                let (color, width) = match (on_path, answer_dead) {
                    (true, true) => (crate::theme::ui_palette().warning, LIT_WIDTH + 2.0),
                    (true, false) => (color, width + 2.5),
                    _ => (color, width),
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

        // "Why?": a click in Why mode, or an Alt+click, asks about the device
        // under the pointer, else the wire.
        let mut asked: Option<Target> = None;
        let alt = ctx.input(|i| i.modifiers.alt);
        if (self.wb.live.why_mode || alt)
            && ctx.input(|i| i.pointer.primary_clicked())
            && let Some(pointer) = ctx.pointer_interact_pos()
            && canvas.contains(pointer)
            && ctx
                .layer_id_at(pointer)
                .is_none_or(|l| l.order == egui::Order::Background)
        {
            let device = project
                .symbols()
                .filter(|s| s.page == page && s.pins.len() >= 2)
                .filter_map(|s| {
                    let pts: Vec<egui::Pos2> = (0..s.pins.len())
                        .filter_map(|p| s.pin_position(p as u16))
                        .map(|p| to_screen(egui::vec2(p.x as f32, p.y as f32)))
                        .collect();
                    let rect = egui::Rect::from_points(&pts).expand(WHY_REACH);
                    rect.contains(pointer)
                        .then_some((rect.width() * rect.height(), s.id))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id)| Target::Symbol(id));
            let wire = || {
                wires
                    .iter()
                    .find(|w| {
                        w.points.windows(2).any(|seg| {
                            distance_to_segment(pointer, to_screen(seg[0]), to_screen(seg[1]))
                                <= HOVER_REACH
                        })
                    })
                    .map(|w| Target::Wire(w.id))
            };
            asked = device.or_else(wire);
        }

        // Pole badges: a click closes or opens the pole.
        let mut clicked: Option<(SymbolId, usize, bool)> = None;
        let reduced = self.settings.reduce_motion;
        let time = ctx.input(|i| i.time);
        // The first badge of each kind, for lessons to point at.
        let mut pole_rect: Option<egui::Rect> = None;
        let mut coil_rect: Option<egui::Rect> = None;
        let mut output_rect: Option<egui::Rect> = None;
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
                    if break_symbols.contains(&badge.symbol) {
                        // The open contact a "Why?" answer points at.
                        let pulse = if reduced {
                            1.0
                        } else {
                            0.6 + 0.4 * (time * 5.0).sin().abs() as f32
                        };
                        p.circle_stroke(
                            rect.center(),
                            BADGE + 4.0 * pulse,
                            egui::Stroke::new(2.5, crate::theme::ui_palette().warning),
                        );
                    }
                    if badge.mixed {
                        p.circle_stroke(
                            rect.center(),
                            BADGE + 2.0,
                            egui::Stroke::new(1.5, crate::theme::ui_palette().warning),
                        );
                    }
                    if badge.closed {
                        p.circle_filled(rect.center(), BADGE, crate::theme::ui_palette().success);
                    } else {
                        p.circle_filled(rect.center(), BADGE, crate::theme::ui_palette().panel);
                        p.circle_stroke(
                            rect.center(),
                            BADGE,
                            egui::Stroke::new(1.5, crate::theme::ui_palette().text_secondary),
                        );
                    }
                    pole_rect.get_or_insert(rect);
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
        let mut coil_clicked: Option<(SymbolId, bool)> = None;
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
                        p.rect_filled(square, 2.0, crate::theme::ui_palette().success);
                    } else {
                        p.rect_filled(square, 2.0, crate::theme::ui_palette().panel);
                        p.rect_stroke(
                            square,
                            2.0,
                            egui::Stroke::new(1.5, crate::theme::ui_palette().text_secondary),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if badge.output {
                        output_rect.get_or_insert(rect);
                    } else {
                        coil_rect.get_or_insert(rect);
                    }
                    let name = symbol_name(project, badge.symbol);
                    let tip = format!(
                        "{name} · {} {} · click to switch it {}",
                        if badge.output { "output" } else { "coil" },
                        if badge.on { "on" } else { "off" },
                        if badge.on { "off" } else { "on" },
                    );
                    if response.on_hover_text(tip).clicked() {
                        coil_clicked = Some((badge.symbol, badge.output));
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
                        p.circle_filled(
                            rect.center(),
                            BADGE * 0.7,
                            crate::theme::ui_palette().success,
                        );
                    } else {
                        p.circle_stroke(
                            rect.center(),
                            BADGE * 0.7,
                            egui::Stroke::new(1.5, crate::theme::ui_palette().text_secondary),
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
                        crate::theme::ui_palette().warning.gamma_multiply(0.18),
                    );
                    p.circle_filled(
                        rect.center(),
                        BADGE * 1.5,
                        crate::theme::ui_palette().warning.gamma_multiply(0.35),
                    );
                });
        }

        let closed = badges.iter().filter(|b| b.closed).count();
        let mut open_all = false;
        let mut leave = false;
        let mut open_plc = false;
        let plc_running = self.wb.live.plc.is_running();
        let mut open_motor: Option<SymbolId> = None;
        let mut toggle_why = false;
        let mut clear_why = false;
        let mut follow: Option<SymbolId> = None;
        let why_mode = self.wb.live.why_mode;
        let operators = operators(project, page, &poles);
        let card = super::panel::Panel::new(
            "live_card",
            "Live potentials",
            canvas,
            super::panel::Side::Left,
            240.0,
        )
        .top(10.0)
        .fixed_open()
        .fit()
        .show(ctx, |ui| {
            if ui
                .small_button("✕ Close")
                .on_hover_text(super::tips::hover(ui.ctx(), "Close", "sim.live"))
                .clicked()
            {
                leave = true;
            }
            if badges.is_empty() {
                ui.weak("No switched contacts on this page");
            } else {
                ui.add(egui::Label::new(egui::RichText::new(&story).strong()).wrap())
                    .on_hover_text(format!("{closed} of {} poles closed", badges.len()));
                if ui
                    .small_button("Reset")
                    .on_hover_text("Back to the plant at rest")
                    .clicked()
                {
                    open_all = true;
                }
            }
            if !operators.is_empty() {
                ui.add_space(2.0);
                ui.label(egui::RichText::new("Operate").strong());
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .id_salt("live_operate")
                    .show(ui, |ui| {
                        for op in &operators {
                            ui.horizontal(|ui| {
                                ui.label(&op.label);
                                ui.weak(format!("{} · {}", op.kind, op.state));
                            });
                            let response = ui.add(egui::Button::new(op.action).selected(op.active));
                            let tip = if op.momentary {
                                "Hold the mouse button down; it springs back when you let go"
                            } else {
                                "Click to switch; it stays until you click again"
                            };
                            let response = response.on_hover_text(tip);
                            if op.momentary {
                                if response.is_pointer_button_down_on() {
                                    pressed = Some((op.symbol, 0));
                                }
                            } else if response.clicked() {
                                clicked = Some((op.symbol, 0, true));
                            }
                        }
                    });
            }
            ui.horizontal(|ui| {
                if ui
                    .small_button("PLC program…")
                    .on_hover_text("Run a ladder or Structured Text program on the drawn I/O")
                    .clicked()
                {
                    open_plc = true;
                }
                if plc_running {
                    ui.colored_label(crate::theme::ui_palette().success, "running");
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
                    crate::theme::ui_palette().error,
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
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(why_mode, "Why?")
                    .on_hover_text(
                        "Then click a wire or a device to see why it is lit or dead. Holding Alt while you click does the same at any time",
                    )
                    .clicked()
                {
                    toggle_why = true;
                }
                if answer.is_some() && ui.small_button("Clear").clicked() {
                    clear_why = true;
                }
            });
            if why_mode && answer.is_none() {
                ui.weak("Click a wire or a device");
            }
            if let Some(a) = &answer {
                ui.add(egui::Label::new(egui::RichText::new(&a.title).strong()).wrap());
                for line in &a.lines {
                    if let Some(symbol) = line.symbol {
                        if ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(&line.text)
                                        .color(crate::theme::ui_palette().accent)
                                        .underline(),
                                )
                                .wrap()
                                .sense(egui::Sense::click()),
                            )
                            .on_hover_text("Ask why about this one")
                            .clicked()
                        {
                            follow = Some(symbol);
                        }
                    } else {
                        ui.add(egui::Label::new(&line.text).wrap());
                    }
                }
                ui.weak("Follows potentials, not currents: no direction of flow is modelled.");
            }
            ui.add_space(2.0);
            open_motor =
                super::simulate::values_block(ui, project, &analysis, &|s| symbol_name(project, s));
            if legend.carried.is_empty() && legend.shorted == 0 {
                ui.weak("Nothing is energized");
            }
        });

        for (id, rect, label) in [
            (
                "live.card",
                card.map(|c| c.response.rect),
                "Live potentials",
            ),
            ("live.pole", pole_rect, "Pole badge"),
            ("live.coil", coil_rect, "Coil badge"),
            ("live.output", output_rect, "Output badge"),
        ] {
            if let Some(rect) = rect {
                self.anchor(id, rect, label);
            }
        }
        // Draws again only when something changed; egui repaints on input.
        if open_plc {
            self.wb.live.plc.window = true;
        }
        if leave
            || open_all
            || toggle_why
            || asked.is_some()
            || follow.is_some()
            || clicked.is_some()
            || timing.is_some()
            || animating
            || plc_running
        {
            ctx.request_repaint();
        }
        if toggle_why {
            self.wb.live.why_mode ^= true;
            self.learn_event(novo_learn::Event::Clicked("live.why".to_owned()));
        }
        if clear_why {
            self.wb.live.why = None;
            self.wb.live.why_mode = false;
        }
        if let Some(target) = asked {
            self.wb.live.why = Some(target);
            self.wb.live.why_mode = false;
        }
        if let Some(symbol) = follow {
            self.wb.live.why = Some(Target::Symbol(symbol));
        }
        if !break_symbols.is_empty() && !self.settings.reduce_motion {
            ctx.request_repaint();
        }
        if let Some(motor) = open_motor {
            self.open_motor_start_for(motor);
        }
        if leave {
            self.wb.live.leave();
        }
        if open_all {
            if let Some(doc) = self.backend.document() {
                self.wb.live.reset_poles(doc.project());
            }
            self.wb.live.forced.clear();
            self.wb.live.clock.reset();
        }
        if let Some((coil, output)) = coil_clicked {
            self.live_coil_clicked(coil, output);
        }
        match pressed {
            Some((symbol, group)) if !driven.contains(&symbol) => {
                self.live_pole_pressed(symbol, group);
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
            self.live_pole_clicked(symbol, group, together);
        }
    }

    /// A click on a coil or output badge: the coil is switched by hand.
    pub(super) fn live_coil_clicked(&mut self, coil: SymbolId, output: bool) {
        self.learn_event(novo_learn::Event::Clicked(
            if output { "live.output" } else { "live.coil" }.to_owned(),
        ));
        if !self.wb.live.forced.remove(&coil) {
            self.wb.live.forced.insert(coil);
        }
    }

    /// A pushbutton's badge goes down: its poles switch until it is let go.
    pub(super) fn live_pole_pressed(&mut self, symbol: SymbolId, group: usize) {
        if self.wb.live.held.is_none() {
            self.learn_event(novo_learn::Event::Clicked("live.pole".to_owned()));
        }
        if let Some(doc) = self.backend.document() {
            self.wb.live.press(doc.project(), symbol, group);
        }
    }

    /// A click on a pole badge: `together` flips every pole of the symbol
    /// (a plain click), else this pole only (Shift+click).
    pub(super) fn live_pole_clicked(&mut self, symbol: SymbolId, group: usize, together: bool) {
        self.learn_event(novo_learn::Event::Clicked("live.pole".to_owned()));
        if together {
            if let Some(doc) = self.backend.document() {
                self.wb.live.flip_together(doc.project(), symbol, group);
            }
        } else {
            self.wb.live.flip(symbol, group);
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

    fn part(
        tx: &mut novo_core::Tx<'_>,
        page: PageId,
        key: &str,
        y: f64,
        role: SymbolRole,
    ) -> SymbolId {
        let id = tx
            .place_symbol(
                page,
                key,
                Placement::at(Point::new(0.0, y)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                role,
            )
            .unwrap();
        if key.contains("contact") {
            tx.update::<novo_core::SymbolInstance>(id, |s| s.pass_through = vec![vec![0, 1]])
                .unwrap();
        }
        id
    }

    /// Start button, hold contact of -K1 in parallel, coil -K1.
    fn seal_in(doc: &mut Document) -> (SymbolId, SymbolId, SymbolId, novo_core::DeviceId) {
        let (out, _) = doc
            .transact("Draw", |tx| {
                let page = tx.add_page("1")?;
                let l1 = tx.add_potential("L1", PotentialKind::Line)?;
                let n = tx.add_potential("N", PotentialKind::Neutral)?;
                let k1 = tx.add_device(novo_core::DeviceTag::product("K1"), "")?;
                let feed = part(
                    tx,
                    page,
                    "t/feed",
                    0.0,
                    SymbolRole::PotentialFeed { potential: l1 },
                );
                let neutral = part(
                    tx,
                    page,
                    "t/feed",
                    10.0,
                    SymbolRole::PotentialFeed { potential: n },
                );
                let start = part(tx, page, "t/contact/no", 20.0, SymbolRole::Unassigned);
                // Not -K1's yet: only the hand moves it.
                let hold = part(tx, page, "t/contact/no", 30.0, SymbolRole::Unassigned);
                let role = SymbolRole::Device {
                    device: k1,
                    main: true,
                };
                let coil = part(tx, page, "t/coil", 40.0, role);
                tx.connect(Endpoint::pin(feed, 1), Endpoint::pin(start, 0))?;
                tx.connect(Endpoint::pin(feed, 1), Endpoint::pin(hold, 0))?;
                tx.connect(Endpoint::pin(start, 1), Endpoint::pin(coil, 0))?;
                tx.connect(Endpoint::pin(hold, 1), Endpoint::pin(coil, 0))?;
                tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(neutral, 1))?;
                Ok((start, hold, coil, k1))
            })
            .unwrap();
        out
    }

    /// A contact closed by hand before it belonged to the coil must not
    /// stay latched once the coil drops out.
    #[test]
    fn a_contact_follows_its_coil_not_an_old_click() {
        let mut doc = Document::default();
        let (start, hold, coil, k1) = seal_in(&mut doc);
        let mut live = LiveView::default();
        let state = |live: &mut LiveView, doc: &Document| {
            let s = live.solved(doc.project(), 0.0);
            (
                s.settled.energized.contains(&coil),
                s.settled.poles.is_closed(hold, 0),
            )
        };
        // The stale click: the hold contact closed by hand feeds the coil.
        live.flip(hold, 0);
        assert_eq!(state(&mut live, &doc), (true, true));
        // The view is restarted before the contact is assigned.
        live.invalidate();
        // It becomes -K1's contact: the coil is off, so it opens.
        doc.transact("Assign", |tx| {
            tx.update::<novo_core::SymbolInstance>(hold, |s| {
                s.role = SymbolRole::Device {
                    device: k1,
                    main: false,
                };
            })
        })
        .unwrap();
        assert_eq!(state(&mut live, &doc), (false, false), "follows the coil");
        // Press start: the coil pulls in and closes it; released: it holds.
        live.flip(start, 0);
        assert_eq!(state(&mut live, &doc), (true, true));
        live.flip(start, 0);
        assert_eq!(state(&mut live, &doc), (true, true), "sealed in");
        // Breaker open (nothing feeds): both drop and stay down.
        live.poles = PoleState::all_open();
        live.invalidate();
        assert_eq!(state(&mut live, &doc), (false, false));
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

    /// The Operate list holds the hand devices only, with plain-words state.
    #[test]
    fn the_operate_list_shows_buttons_not_coil_driven_contacts() {
        let mut doc = Document::default();
        let (out, _) = doc
            .transact("Draw", |tx| {
                let page = tx.add_page("1")?;
                let s1 = tx.add_device(novo_core::DeviceTag::product("S1"), "")?;
                let k1 = tx.add_device(novo_core::DeviceTag::product("K1"), "")?;
                let dev = |device, main| SymbolRole::Device { device, main };
                let start = part(tx, page, "iec/operators/pushbutton_no", 0.0, dev(s1, true));
                let seal = part(tx, page, "iec/contacts/contact_no", 10.0, dev(k1, false));
                part(tx, page, "iec/relays/coil", 20.0, dev(k1, true));
                tx.update::<novo_core::SymbolInstance>(start, |s| {
                    s.pass_through = vec![vec![0, 1]]
                })?;
                Ok((page, start, seal))
            })
            .unwrap();
        let (page, start, _seal) = out;
        let project = doc.project();
        let rest = novo_sim::rest_state(project);
        let ops = operators(project, page, &rest);
        assert_eq!(ops.len(), 1, "the seal-in contact belongs to a coil");
        assert_eq!(
            (ops[0].symbol, ops[0].state, ops[0].active),
            (start, "released", false)
        );
        let mut pressed = rest.clone();
        pressed.set_all(project, start, true);
        let ops = operators(project, page, &pressed);
        assert_eq!((ops[0].state, ops[0].active), ("pressed", true));
        assert!(ops[0].momentary);
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
