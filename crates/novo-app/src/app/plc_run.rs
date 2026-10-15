//! The PLC program window (Live potentials, "PLC program"): write a
//! Structured Text or ladder program, run it against the drawing, and watch
//! it. While it runs, the PLC reads the drawn input channels, scans, and puts
//! its outputs on the drawn output channels, so a start button wired to DI 0
//! and a contactor coil wired to DQ 0 behave as the program says.
//!
//! Addresses come from the I/O list (the project's address style, except
//! Rockwell, which has no byte addresses). The program is kept in the
//! window and in a file you save; it is not part of the project file yet.

use std::collections::BTreeSet;
use std::path::PathBuf;

use eframe::egui;
use novo_core::{AnyId, Netlist, Project, SymbolId};
use novo_plc::{ChannelType, PlcIndex};
use novo_sim::program::{
    Address, Bench, ClosedLoop, Element, ElementKind, IoMap, Program, Size, Space, Value,
};
use novo_sim::{Clock, PoleState, Settled};
use novo_theme::{tokens, widgets};

use super::{Level, NovoApp};

const EXAMPLE: &str = "\
(* Motor with a seal-in. Addresses are the ones in the I/O list. *)
PROGRAM Motor
VAR
    start   AT %IX0.0 : BOOL;   (* push button, pressed = TRUE *)
    stop_ok AT %IX0.1 : BOOL;   (* NC stop button: TRUE while healthy *)
    run     AT %QX0.0 : BOOL;   (* contactor coil *)
    delay   : TON;
END_VAR

RUNG \"Start with seal-in\"
    { [start] | [run] } -- [stop_ok] -- (run);

(* Structured Text works next to ladder. *)
delay(IN := run, PT := T#3s);
";

/// What the program window remembers.
#[derive(Default)]
pub(crate) struct PlcRun {
    pub(super) window: bool,
    text: String,
    path: Option<PathBuf>,
    /// The parse error of the last Run, with its place.
    error: Option<String>,
    looped: Option<ClosedLoop>,
    running: bool,
    /// What the I/O list could not give the program.
    notes: Vec<String>,
    /// The project revision the channel map was built for.
    map_revision: Option<u64>,
}

/// The channels of the I/O list by address, and what could not be mapped.
fn io_map(project: &Project, index: &PlcIndex) -> (IoMap, Vec<String>) {
    let mut map = IoMap::new();
    let mut notes = Vec::new();
    let mut rockwell = false;
    for row in index.io_rows(project) {
        if row.spare {
            continue;
        }
        let Some((_, AnyId::Symbol(symbol))) = row.drawn_at else {
            continue;
        };
        let Some(address) = row.address.as_deref() else {
            rockwell = true;
            continue;
        };
        let ok = match row.kind {
            ChannelType::DigitalInput => map.add_input(address, symbol),
            ChannelType::DigitalOutput | ChannelType::RelayOutput => {
                map.add_output(address, symbol)
            }
            ChannelType::AnalogInput | ChannelType::AnalogOutput => true,
        };
        if !ok {
            notes.push(format!("{address} is not an address a program can use"));
        }
    }
    if rockwell {
        notes.push(
            "Rockwell addresses are slot and channel, not byte and bit. Set the address style to IEC, Siemens or Plain to run a program."
                .into(),
        );
    }
    (map, notes)
}

impl PlcRun {
    pub(super) fn is_running(&self) -> bool {
        self.running && self.looped.is_some()
    }

    /// The clock timer contacts count on while the program runs.
    pub(super) fn clock(&self) -> Option<&Clock> {
        self.looped
            .as_ref()
            .filter(|_| self.running)
            .map(|l| &l.clock)
    }

    /// The output channels the program has on.
    pub(super) fn outputs_on(&self) -> BTreeSet<SymbolId> {
        self.looped
            .as_ref()
            .filter(|_| self.running)
            .map(|l| l.bench.outputs_on().clone())
            .unwrap_or_default()
    }

    /// One frame: the program scans, the circuit is solved again.
    pub(super) fn step(
        &mut self,
        project: &Project,
        netlist: &Netlist,
        manual: &PoleState,
        forced: &BTreeSet<SymbolId>,
        dt: f64,
    ) -> Option<Settled> {
        let looped = self.looped.as_mut()?;
        Some(looped.step(project, netlist, manual, forced, dt).clone())
    }

    /// Wires the running program to the drawing again when the drawing
    /// changed, keeping the program's state.
    pub(super) fn sync(&mut self, project: &Project, functions: &dyn novo_plc::Functions) {
        if !self.running || self.map_revision == Some(project.revision()) {
            return;
        }
        let Some(looped) = self.looped.as_mut() else {
            return;
        };
        let index = PlcIndex::build(project, functions);
        let (map, notes) = io_map(project, &index);
        looped.bench.set_map(map);
        self.notes = notes;
        self.map_revision = Some(project.revision());
    }

    /// Reads the text and starts it; `Err` is the message to show.
    fn run(&mut self, project: &Project, functions: &dyn novo_plc::Functions) {
        let program = match Program::parse(&self.text) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        self.error = None;
        let index = PlcIndex::build(project, functions);
        let (map, notes) = io_map(project, &index);
        let bench = Bench::new(program, map);
        self.notes = notes;
        for u in bench.unbound() {
            self.notes.push(format!(
                "{} is used by the program but no {} channel is drawn there",
                u.address,
                if u.is_input { "input" } else { "output" }
            ));
        }
        self.looped = Some(ClosedLoop::new(bench));
        self.running = true;
        self.map_revision = Some(project.revision());
    }
}

impl NovoApp {
    /// The `sim.plc_program` command: live view on, program window open.
    pub(super) fn open_plc_program(&mut self) {
        if !self.wb.live.on {
            self.toggle_live();
        }
        self.wb.live.plc.window = true;
    }

    fn plc_open_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PLC program", &["st", "ld", "txt"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let plc = &mut self.wb.live.plc;
                plc.text = text;
                plc.path = Some(path);
                plc.error = None;
            }
            Err(e) => self.message(Level::Error, format!("Could not read the program: {e}")),
        }
    }

    fn plc_save_file(&mut self) {
        let plc = &self.wb.live.plc;
        let mut dialog = rfd::FileDialog::new().add_filter("PLC program", &["st"]);
        if let Some(name) = plc.path.as_ref().and_then(|p| p.file_name()) {
            dialog = dialog.set_file_name(name.to_string_lossy());
        } else {
            dialog = dialog.set_file_name("program.st");
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        match std::fs::write(&path, &self.wb.live.plc.text) {
            Ok(()) => {
                self.message(Level::Info, format!("Saved {}", path.display()));
                self.wb.live.plc.path = Some(path);
            }
            Err(e) => self.message(Level::Error, format!("Could not save the program: {e}")),
        }
    }

    /// The PLC program window, drawn over the live view.
    pub(super) fn plc_window(&mut self, ctx: &egui::Context) {
        if !self.wb.live.plc.window {
            return;
        }
        let mut open = true;
        let mut action = None::<PlcAction>;
        let plc = &mut self.wb.live.plc;
        egui::Window::new("PLC program")
            .open(&mut open)
            .default_size([560.0, 640.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Open…").clicked() {
                        action = Some(PlcAction::Open);
                    }
                    if ui.button("Save as…").clicked() {
                        action = Some(PlcAction::Save);
                    }
                    if ui
                        .button("Example")
                        .on_hover_text("A motor with a seal-in and a timer")
                        .clicked()
                    {
                        plc.text = EXAMPLE.to_owned();
                    }
                    if let Some(p) = &plc.path {
                        ui.weak(
                            p.file_name()
                                .map(|n| n.to_string_lossy())
                                .unwrap_or_default(),
                        );
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt("plc_text")
                    .max_height(260.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut plc.text)
                                .code_editor()
                                .desired_rows(12)
                                .desired_width(f32::INFINITY)
                                .hint_text("Structured Text or ladder; press Example to see both"),
                        );
                    });
                ui.horizontal(|ui| {
                    if plc.running {
                        if widgets::primary_button(ui, "■ Stop").clicked() {
                            action = Some(PlcAction::Stop);
                        }
                        if ui.button("↻ Reload and restart").clicked() {
                            action = Some(PlcAction::Run);
                        }
                    } else if widgets::primary_button(ui, "▶ Run").clicked() {
                        action = Some(PlcAction::Run);
                    }
                    if let Some(l) = plc.looped.as_ref().filter(|_| plc.running) {
                        ui.weak(format!(
                            "{} scans · {} outputs on",
                            l.bench.runtime().scans(),
                            l.bench.outputs_on().len()
                        ));
                    }
                });
                if let Some(e) = &plc.error {
                    ui.colored_label(tokens::ERROR, e);
                }
                if let Some(f) = plc
                    .looped
                    .as_ref()
                    .filter(|_| plc.running)
                    .and_then(|l| l.bench.fault())
                {
                    ui.colored_label(tokens::ERROR, format!("Program fault, outputs held: {f}"));
                }
                for n in &plc.notes {
                    ui.colored_label(tokens::WARNING, n);
                }
                if let Some(looped) = plc.looped.as_mut().filter(|_| plc.running) {
                    monitor(ui, looped);
                }
            });
        if !open {
            self.wb.live.plc.window = false;
        }
        match action {
            Some(PlcAction::Open) => self.plc_open_file(),
            Some(PlcAction::Save) => self.plc_save_file(),
            Some(PlcAction::Stop) => {
                self.wb.live.plc.running = false;
                self.wb.live.invalidate();
            }
            Some(PlcAction::Run) => {
                let (Some(doc), Some(view)) =
                    (self.backend.document(), self.canvas.schematic_ref())
                else {
                    return;
                };
                let project = doc.project();
                self.wb.live.plc.run(project, view.editor.catalog.library());
                self.wb.live.invalidate();
            }
            None => {}
        }
    }
}

enum PlcAction {
    Open,
    Save,
    Stop,
    Run,
}

/// The ladder with power flow, and the variables.
fn monitor(ui: &mut egui::Ui, looped: &mut ClosedLoop) {
    let runtime = looped.bench.runtime();
    let rungs = runtime.program().rungs.clone();
    if !rungs.is_empty() {
        ui.separator();
        widgets::section_label(ui, "Ladder");
        for rung in &rungs {
            ui.weak(&rung.title);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                elements(ui, runtime, &rung.elements);
            });
        }
    }
    ui.separator();
    widgets::section_label(ui, "Variables");
    let watch = runtime.watch();
    let words: Vec<Address> = runtime
        .program()
        .addresses
        .iter()
        .copied()
        .filter(|a| a.space == Space::Input && a.size != Size::Bit)
        .collect();
    egui::Grid::new("plc_watch").striped(true).show(ui, |ui| {
        for w in &watch {
            ui.label(&w.name);
            ui.weak(&w.ty);
            ui.monospace(&w.value);
            ui.weak(w.address.map(|a| a.to_string()).unwrap_or_default());
            ui.end_row();
        }
    });
    if !words.is_empty() {
        ui.add_space(4.0);
        widgets::section_label(ui, "Analog inputs (set by hand)");
        for a in words {
            let mut v = looped.bench.runtime().input(a).as_int();
            ui.horizontal(|ui| {
                ui.monospace(a.to_string());
                if ui.add(egui::DragValue::new(&mut v).speed(10.0)).changed() {
                    looped.bench.runtime_mut().set_input(a, Value::Int(v));
                }
            });
        }
    }
}

fn elements(ui: &mut egui::Ui, runtime: &novo_sim::program::Runtime, list: &[Element]) {
    for (i, e) in list.iter().enumerate() {
        if i > 0 {
            ui.weak("—");
        }
        let on = runtime.power(e.probe);
        let colored = |text: String| {
            let t = egui::RichText::new(text).monospace();
            if on {
                t.color(tokens::SUCCESS)
            } else {
                t.color(tokens::TEXT_FAINT)
            }
        };
        match &e.kind {
            ElementKind::Contact { text } => {
                ui.label(colored(format!("[{text}]")));
            }
            ElementKind::Coil { text } => {
                ui.label(colored(format!("({text})")));
            }
            ElementKind::Box { text } => {
                ui.label(colored(format!("<{text}>")));
            }
            ElementKind::Branch(branches) => {
                ui.label(colored("{".into()));
                for (n, b) in branches.iter().enumerate() {
                    if n > 0 {
                        ui.weak("|");
                    }
                    elements(ui, runtime, b);
                }
                ui.label(colored("}".into()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{DeviceKind, DeviceTag, Document, Endpoint, Pin, Placement, Point};
    use novo_core::{PotentialKind, SymbolRole};
    use novo_plc::FromKey;
    use novo_plc::card::{CHANNEL, CHANNELS, START_INPUT, START_OUTPUT};

    struct Plant {
        doc: Document,
        start: SymbolId,
        stop: SymbolId,
        coil: SymbolId,
    }

    /// A real PLC card `-A1` (DI 0, DI 1, DQ 0) with a start button on DI 0,
    /// a normally closed stop on DI 1 and a contactor coil on DQ 0.
    fn plant() -> Plant {
        let mut doc = Document::default();
        let ((start, stop, coil), _) = doc
            .transact("Draw", |tx| {
                let page = tx.add_page("1")?;
                let v24 = tx.add_potential("24V", PotentialKind::DcPositive)?;
                let v0 = tx.add_potential("0V", PotentialKind::DcNegative)?;
                let card = tx.add_device_of_kind(DeviceTag::parse("-A1")?, "", DeviceKind::Plc)?;
                tx.set_property(card, CHANNELS, Some(novo_core::Value::from("DI2 DQ1")))?;
                tx.set_property(card, START_INPUT, Some(novo_core::Value::Int(0)))?;
                tx.set_property(card, START_OUTPUT, Some(novo_core::Value::Int(0)))?;
                let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
                let s1 = tx.add_device(DeviceTag::parse("-S1")?, "")?;
                let s2 = tx.add_device(DeviceTag::parse("-S2")?, "")?;
                let dev = |device| SymbolRole::Device {
                    device,
                    main: false,
                };
                let put = |tx: &mut novo_core::Tx<'_>, key: &str, y: f64, pins: usize, role| {
                    let pins = (0..pins)
                        .map(|i| Pin::new(format!("{}", i + 1), 0.0, i as f64 * 8.0))
                        .collect();
                    let id =
                        tx.place_symbol(page, key, Placement::at(Point::new(0.0, y)), pins, role)?;
                    if key.contains("contact") {
                        tx.update::<novo_core::SymbolInstance>(id, |s| {
                            s.pass_through = vec![vec![0, 1]];
                        })?;
                    }
                    Ok::<_, novo_core::ModelError>(id)
                };
                let f24 = put(
                    tx,
                    "t/feed",
                    0.0,
                    2,
                    SymbolRole::PotentialFeed { potential: v24 },
                )?;
                let f0 = put(
                    tx,
                    "t/feed",
                    10.0,
                    2,
                    SymbolRole::PotentialFeed { potential: v0 },
                )?;
                let start = put(tx, "t/contact/no", 20.0, 2, dev(s1))?;
                let stop = put(tx, "t/contact/nc", 30.0, 2, dev(s2))?;
                let di0 = put(tx, "iec/plc/digital_input", 40.0, 1, dev(card))?;
                let di1 = put(tx, "iec/plc/digital_input", 50.0, 1, dev(card))?;
                let dq0 = put(tx, "iec/plc/digital_output", 60.0, 1, dev(card))?;
                for (s, n) in [(di0, "DI 0"), (di1, "DI 1"), (dq0, "DQ 0")] {
                    tx.set_property(s, CHANNEL, Some(novo_core::Value::from(n)))?;
                }
                let coil = put(tx, "t/coil", 70.0, 2, dev(k1))?;
                tx.connect(Endpoint::pin(f24, 1), Endpoint::pin(start, 0))?;
                tx.connect(Endpoint::pin(f24, 1), Endpoint::pin(stop, 0))?;
                tx.connect(Endpoint::pin(start, 1), Endpoint::pin(di0, 0))?;
                tx.connect(Endpoint::pin(stop, 1), Endpoint::pin(di1, 0))?;
                tx.connect(Endpoint::pin(dq0, 0), Endpoint::pin(coil, 0))?;
                tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(f0, 1))?;
                Ok((start, stop, coil))
            })
            .unwrap();
        Plant {
            doc,
            start,
            stop,
            coil,
        }
    }

    fn scans(plc: &mut PlcRun, p: &Plant, hand: &PoleState, seconds: f64) -> Settled {
        let project = p.doc.project();
        let netlist = project.netlist();
        let mut last = None;
        for _ in 0..(seconds / 0.02).round() as usize {
            last = plc.step(project, &netlist, hand, &BTreeSet::new(), 0.02);
        }
        last.expect("the program is running")
    }

    #[test]
    fn the_example_program_runs_on_the_io_list_of_a_drawn_card() {
        let p = plant();
        let project = p.doc.project();
        let mut plc = PlcRun {
            text: EXAMPLE.to_owned(),
            ..PlcRun::default()
        };
        plc.run(project, &FromKey);
        assert_eq!(plc.error, None);
        assert!(plc.is_running());
        assert!(plc.notes.is_empty(), "{:?}", plc.notes);

        let mut hand = PoleState::all_open();
        hand.set_all(project, p.stop, true); // NC stop, at rest
        assert!(!scans(&mut plc, &p, &hand, 0.2).energized.contains(&p.coil));

        hand.set_all(project, p.start, true);
        assert!(scans(&mut plc, &p, &hand, 0.2).energized.contains(&p.coil));
        hand.set_all(project, p.start, false);
        assert!(
            scans(&mut plc, &p, &hand, 1.0).energized.contains(&p.coil),
            "the seal-in holds it"
        );
        hand.set_all(project, p.stop, false);
        assert!(!scans(&mut plc, &p, &hand, 0.2).energized.contains(&p.coil));
    }

    #[test]
    fn a_mistake_is_reported_and_nothing_starts() {
        let p = plant();
        let mut plc = PlcRun {
            text: "VAR a : BOOL; END_VAR\nb := a;".into(),
            ..PlcRun::default()
        };
        plc.run(p.doc.project(), &FromKey);
        assert!(!plc.is_running());
        let e = plc.error.as_deref().unwrap();
        assert!(e.contains("line 2") && e.contains("`b` is not declared"), "{e}");
    }

    #[test]
    fn addresses_with_no_drawn_channel_are_listed() {
        let p = plant();
        let mut plc = PlcRun {
            text: "VAR x AT %IX5.5 : BOOL; q AT %QX2.0 : BOOL; END_VAR q := x;".into(),
            ..PlcRun::default()
        };
        plc.run(p.doc.project(), &FromKey);
        assert!(plc.is_running());
        assert_eq!(plc.notes.len(), 2, "{:?}", plc.notes);
    }
}
