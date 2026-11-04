//! The parts pass: right after devices are placed, the Devices drawer shows
//! only the devices that have no part yet, sorted by class (`K`, `M`, `Q`
//! ...), each with a part already chosen: the part last used for that class,
//! else the best fit. Enter takes the highlighted row, Down moves on without
//! taking it, Accept all takes every row in one undo step.
//!
//! The offer is an ordinary toast with a "Choose parts" button, never a
//! problem marker (docs/design/problem-triangles.md).

use std::collections::HashMap;

use eframe::egui;
use novo_core::{DeviceId, DeviceKind, Project, natural_cmp};
use novo_parts::{Catalog, PartKey, assign_choices, suggestions_for_device};

use super::drawers::DrawerAction;
use super::workbench::Drawer;
use super::{Level, NovoApp};

/// Most parts one row's list offers; the Parts window has the rest.
const OPTIONS: usize = 8;
/// How many suggestions are searched for the remembered part.
const REMEMBER_DEPTH: usize = 64;
/// How many points below the best fit a remembered part may be.
const CLOSE: u8 = 10;

/// The pass while it is open.
#[derive(Debug, Default)]
pub(crate) struct PartsPass {
    /// The highlighted row.
    cursor: usize,
    /// Parts picked from a row's list, over the preselection.
    picks: HashMap<DeviceId, PartKey>,
}

/// What a click or key in the pass asks for.
#[derive(Debug)]
pub(super) enum PassAction {
    /// Takes the part of row `n`.
    Accept(usize),
    AcceptAll,
    /// Moves the highlight by this many rows.
    Move(isize),
    Select(usize),
    Pick(DeviceId, PartKey),
    Close,
}

/// One device in the pass.
pub(super) struct PassRow {
    device: DeviceId,
    pub(super) tag: String,
    description: String,
    /// `Manufacturer order-number` of each offered part.
    options: Vec<(PartKey, String)>,
    /// Index into `options`; `None` when no catalog part fits.
    chosen: Option<usize>,
}

/// Devices that still need a part, by class then tag.
fn open_devices(project: &Project) -> Vec<DeviceId> {
    let mut devices: Vec<_> = project
        .devices()
        .filter(|d| d.parts.is_empty() && d.kind != DeviceKind::TerminalStrip)
        .collect();
    devices.sort_by(|a, b| {
        a.tag
            .letter_code()
            .cmp(b.tag.letter_code())
            .then_with(|| natural_cmp(&a.tag.to_string(), &b.tag.to_string()))
    });
    devices.iter().map(|d| d.id).collect()
}

/// How many devices have no part (terminal strips excluded).
pub(super) fn open_count(project: &Project) -> usize {
    open_devices(project).len()
}

/// The part last used for the class: what was assigned in this session, else
/// the part of the last device of that class in the project that has one.
fn last_used(project: &Project, class: &str, memory: &HashMap<String, PartKey>) -> Option<PartKey> {
    if let Some(key) = memory.get(class) {
        return Some(key.clone());
    }
    let mut same: Vec<_> = project
        .devices()
        .filter(|d| d.tag.letter_code() == class && !d.parts.is_empty())
        .collect();
    same.sort_by(|a, b| natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
    let part = project.part(*same.last()?.parts.first()?)?;
    Some(PartKey::new(&part.manufacturer, &part.order_number))
}

/// The rows of the pass, with the part each would get.
pub(super) fn pass_rows(
    project: &Project,
    catalog: &Catalog,
    pass: &PartsPass,
    memory: &HashMap<String, PartKey>,
) -> Vec<PassRow> {
    open_devices(project)
        .into_iter()
        .filter_map(|id| {
            let device = project.device(id)?;
            let class = device.tag.letter_code().to_owned();
            // The device's own suggestions, best first: the remembered part
            // counts only if it is among them and fits nearly as well as the
            // best (a fuse link last used for -F1 is not proposed to the
            // breaker -F3 that shares the letter code).
            let all = suggestions_for_device(catalog, project, id, "", REMEMBER_DEPTH);
            let best = all.first().map_or(0, |s| s.fit.percent);
            let title =
                |p: &novo_parts::CatalogPart| format!("{} {}", p.manufacturer, p.order_number);
            let mut options: Vec<(PartKey, String)> = all
                .iter()
                .take(OPTIONS)
                .map(|s| (s.part.key(), title(s.part)))
                .collect();
            let wanted = pass.picks.get(&id).cloned().or_else(|| {
                let key = last_used(project, &class, memory)?;
                let s = all.iter().find(|s| s.part.key() == key)?;
                (s.fit.percent + CLOSE >= best).then_some(key)
            });
            let chosen = match wanted {
                Some(key) => Some(options.iter().position(|(k, _)| *k == key).unwrap_or_else(
                    || {
                        let part = catalog.get(&key).expect("a suggestion");
                        options.insert(0, (key, title(part)));
                        0
                    },
                )),
                None => (!options.is_empty()).then_some(0),
            };
            Some(PassRow {
                device: id,
                tag: device.tag.to_string(),
                description: device.description.clone(),
                options,
                chosen,
            })
        })
        .collect()
}

/// The pass as the Devices tab shows it.
pub(super) fn pass_tab(
    ui: &mut egui::Ui,
    rows: &[PassRow],
    cursor: usize,
    actions: &mut Vec<DrawerAction>,
) {
    let cursor = cursor.min(rows.len().saturating_sub(1));
    let mut push = |a: PassAction| actions.push(DrawerAction::Pass(a));
    ui.horizontal(|ui| {
        ui.label(format!("{} without a part", rows.len()));
        let ready = rows.iter().filter(|r| r.chosen.is_some()).count();
        ui.add_enabled_ui(ready > 0, |ui| {
            if ui.button(format!("Accept all ({ready})")).clicked() {
                push(PassAction::AcceptAll);
            }
        });
        ui.weak("?").on_hover_text(
            "Enter takes the part on the highlighted row. Down skips to the next device. \
             The part shown is the one you used last for that kind of device.",
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Done").clicked() {
                push(PassAction::Close);
            }
        });
    });
    if !ui.ctx().egui_wants_keyboard_input() {
        let (enter, down, up) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            )
        });
        if enter {
            push(PassAction::Accept(cursor));
        } else if down {
            push(PassAction::Move(1));
        } else if up {
            push(PassAction::Move(-1));
        }
    }
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("parts_pass")
            .striped(true)
            .num_columns(3)
            .spacing([24.0, 4.0])
            .show(ui, |ui| {
                for head in ["Device", "Description", "Proposed part"] {
                    ui.small(head);
                }
                ui.end_row();
                for (n, row) in rows.iter().enumerate() {
                    let tag = ui.selectable_label(n == cursor, &row.tag);
                    if n == cursor && ui.input(|i| i.pointer.any_down()) {
                        tag.scroll_to_me(None);
                    }
                    if tag.clicked() {
                        push(PassAction::Select(n));
                    }
                    ui.label(&row.description);
                    match row.chosen {
                        None => {
                            ui.weak("No catalog part fits");
                        }
                        Some(at) => {
                            let mut picked = at;
                            egui::ComboBox::from_id_salt(("pass_part", row.device))
                                .selected_text(&row.options[at].1)
                                .show_ui(ui, |ui| {
                                    for (i, (_, title)) in row.options.iter().enumerate() {
                                        ui.selectable_value(&mut picked, i, title);
                                    }
                                });
                            if picked != at {
                                push(PassAction::Pick(row.device, row.options[picked].0.clone()));
                            }
                        }
                    }
                    ui.end_row();
                }
            });
    });
}

impl NovoApp {
    /// After devices were placed: a toast offering the pass when some have
    /// no part. Nothing is shown when every device has one.
    pub(super) fn offer_parts_pass(&mut self) {
        let Some(count) = self
            .backend
            .document()
            .map(|d| open_count(d.project()))
            .filter(|&n| n > 0)
        else {
            return;
        };
        let text = if count == 1 {
            "1 device without a part".to_owned()
        } else {
            format!("{count} devices without a part")
        };
        self.raise_action(text, super::MessageAction::ChooseParts);
    }

    /// Opens the Devices drawer on the devices without a part.
    pub(super) fn start_parts_pass(&mut self) {
        if !self.is_open() {
            return;
        }
        if let Some(problem) = self.wb.part_catalog.ensure() {
            self.message(Level::Error, problem);
        }
        self.wb.parts_pass = Some(PartsPass::default());
        self.wb.drawer = Some(Drawer::Devices);
    }

    /// The rows to show, `None` while no pass is open.
    pub(super) fn parts_pass_rows(&self) -> Option<(Vec<PassRow>, usize)> {
        let pass = self.wb.parts_pass.as_ref()?;
        let project = self.backend.document()?.project();
        let catalog = self.wb.part_catalog.get()?;
        let rows = pass_rows(project, catalog, pass, &self.wb.last_part_by_class);
        Some((rows, pass.cursor))
    }

    /// Remembers `key` as the part last used for each device's class.
    pub(super) fn remember_part(&mut self, devices: &[DeviceId], key: &PartKey) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        for &id in devices {
            if let Some(d) = project.device(id) {
                self.wb
                    .last_part_by_class
                    .insert(d.tag.letter_code().to_owned(), key.clone());
            }
        }
    }

    pub(super) fn pass_action(&mut self, action: PassAction) {
        let Some((rows, cursor)) = self.parts_pass_rows() else {
            return;
        };
        match action {
            PassAction::Close => self.wb.parts_pass = None,
            PassAction::Select(n) => {
                if let Some(p) = &mut self.wb.parts_pass {
                    p.cursor = n;
                }
            }
            PassAction::Move(by) => {
                if let Some(p) = &mut self.wb.parts_pass {
                    let last = rows.len().saturating_sub(1) as isize;
                    p.cursor = (cursor as isize + by).clamp(0, last) as usize;
                }
            }
            PassAction::Pick(device, key) => {
                if let Some(p) = &mut self.wb.parts_pass {
                    p.picks.insert(device, key);
                }
            }
            PassAction::Accept(n) => {
                let Some((row, at)) = rows.get(n).and_then(|r| Some((r, r.chosen?))) else {
                    // Nothing fits this device: move on.
                    self.pass_action(PassAction::Move(1));
                    return;
                };
                let key = row.options[at].0.clone();
                self.apply_pass(&[(row.device, key)]);
            }
            PassAction::AcceptAll => {
                let all: Vec<_> = rows
                    .iter()
                    .filter_map(|r| Some((r.device, r.options[r.chosen?].0.clone())))
                    .collect();
                self.apply_pass(&all);
            }
        }
    }

    /// Assigns the parts as one undo step, remembers them per class and
    /// closes the pass when nothing is left.
    fn apply_pass(&mut self, picks: &[(DeviceId, PartKey)]) {
        let Some(command) = self.backend.document().and_then(|d| {
            let catalog = self.wb.part_catalog.get()?;
            let choices: Vec<_> = picks
                .iter()
                .filter_map(|(id, key)| Some((*id, catalog.get(key)?)))
                .collect();
            assign_choices(d.project(), &choices)
        }) else {
            return;
        };
        match self.execute(command) {
            Some(Ok(_)) => {
                for (id, key) in picks {
                    self.remember_part(&[*id], key);
                }
                let left = self
                    .backend
                    .document()
                    .map_or(0, |d| open_count(d.project()));
                if left == 0 {
                    self.wb.parts_pass = None;
                    self.message(Level::Info, "Every device has a part");
                }
            }
            Some(Err(e)) => self.message(Level::Error, format!("Could not set the parts: {e}")),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::gate1_tests::Drive;
    use novo_core::DeviceTag;
    use novo_parts::CatalogPart;

    fn two_contactors_catalog() -> Catalog {
        let mut c = Catalog::new();
        for n in ["C-1", "C-2"] {
            let mut p = CatalogPart::new("Acme", n);
            p.letter_codes = vec!["K".into()];
            c.upsert(p);
        }
        c
    }

    fn drive() -> (tempfile::TempDir, Drive) {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, crate::app::Action::NewProject);
        d.frame();
        let doc = d.app.backend.document_mut().unwrap();
        doc.transact("Add", |tx| {
            for tag in ["-K1", "-K2", "-K3"] {
                tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
            }
            Ok(())
        })
        .unwrap();
        d.app.wb.part_catalog.set_for_test(two_contactors_catalog());
        (dir, d)
    }

    #[test]
    fn the_offer_names_the_count_and_opens_the_filtered_drawer() {
        let (_dir, mut d) = drive();
        d.app.offer_parts_pass();
        let toast = d.app.messages.last().unwrap();
        assert_eq!(toast.text, "3 devices without a part");
        assert_eq!(toast.action, Some(crate::app::MessageAction::ChooseParts));
        d.app.start_parts_pass();
        assert_eq!(d.app.wb.drawer, Some(Drawer::Devices));
        let (rows, cursor) = d.app.parts_pass_rows().unwrap();
        assert_eq!((rows.len(), cursor), (3, 0));
    }

    #[test]
    fn the_part_used_last_for_the_class_is_preselected() {
        let (_dir, mut d) = drive();
        d.app.start_parts_pass();
        // Without a history the best fit is the first one.
        let (rows, _) = d.app.parts_pass_rows().unwrap();
        assert_eq!(rows[0].options[rows[0].chosen.unwrap()].1, "Acme C-1");
        // The person picks C-2 for -K1 and takes it with Enter.
        let key = PartKey::new("Acme", "C-2");
        d.app.pass_action(PassAction::Pick(rows[0].device, key));
        d.app.pass_action(PassAction::Accept(0));
        // The rest now start on C-2: it was the last one used for K.
        let (rows, _) = d.app.parts_pass_rows().unwrap();
        assert_eq!(rows.len(), 2);
        for r in &rows {
            assert_eq!(r.options[r.chosen.unwrap()].1, "Acme C-2");
        }
    }

    #[test]
    fn accept_all_is_one_undo_step_and_closes_the_pass() {
        let (_dir, mut d) = drive();
        d.app.start_parts_pass();
        d.app.pass_action(PassAction::AcceptAll);
        assert!(d.app.wb.parts_pass.is_none());
        let project = d.app.backend.document().unwrap().project();
        assert_eq!(open_count(project), 0);
        d.app.backend.document_mut().unwrap().undo().unwrap();
        let project = d.app.backend.document().unwrap().project();
        assert_eq!(open_count(project), 3);
        assert!(project.devices().all(|dev| dev.parts.is_empty()));
    }

    #[test]
    fn nothing_is_offered_when_every_device_has_a_part() {
        let (_dir, mut d) = drive();
        d.app.start_parts_pass();
        d.app.pass_action(PassAction::AcceptAll);
        let before = d.app.messages.len();
        d.app.offer_parts_pass();
        assert_eq!(d.app.messages.len(), before);
    }

    #[test]
    fn down_moves_on_without_taking_the_part() {
        let (_dir, mut d) = drive();
        d.app.start_parts_pass();
        d.app.pass_action(PassAction::Move(1));
        assert_eq!(d.app.wb.parts_pass.as_ref().unwrap().cursor, 1);
        d.app.pass_action(PassAction::Move(-5));
        assert_eq!(d.app.wb.parts_pass.as_ref().unwrap().cursor, 0);
        let project = d.app.backend.document().unwrap().project();
        assert_eq!(open_count(project), 3);
    }

    #[test]
    fn clicking_the_toast_button_shows_the_proposed_parts_in_the_drawer() {
        let mut lv = crate::app::lesson_verify_cables::Lv::blank();
        let doc = lv.d.app.backend.document_mut().unwrap();
        doc.transact("Add", |tx| {
            tx.add_device(DeviceTag::parse("-K1").unwrap(), "Pump")
        })
        .unwrap();
        lv.d.app
            .wb
            .part_catalog
            .set_for_test(two_contactors_catalog());
        lv.d.app.offer_parts_pass();
        lv.frames(2);
        assert!(!lv.has_label("Accept all (1)"));
        lv.click_label("Choose parts");
        assert!(lv.has_label("Accept all (1)"), "the drawer lists the rows");
        assert!(lv.has_label("-K1"));
    }

    #[test]
    fn a_fuse_link_used_for_a_fuse_is_not_proposed_to_a_breaker() {
        let (_dir, mut d) = drive_blank();
        let fuse = d.place("fuse", 40.0, 48.0);
        let breaker = d.place("mcb3", 120.0, 48.0);
        let project = d.app.backend.document().unwrap().project();
        let dev = |s| project.symbol(s).unwrap().device().unwrap();
        let (f, b) = (dev(fuse), dev(breaker));
        eprintln!(
            "TAGS {} {}",
            project.device(f).unwrap().tag,
            project.device(b).unwrap().tag
        );
        let mut catalog = Catalog::new();
        for (n, function, letters) in [
            ("FUSE-1", "Fuse holder", ["F", "Q"]),
            ("MCB-1", "Miniature circuit breaker", ["F", "Q"]),
        ] {
            let mut p = CatalogPart::new("Acme", n);
            p.function = function.into();
            p.letter_codes = letters.iter().map(|l| (*l).to_owned()).collect();
            catalog.upsert(p);
        }
        let mut memory = HashMap::new();
        let class_of = |id| project.device(id).unwrap().tag.letter_code().to_owned();
        memory.insert(class_of(f), PartKey::new("Acme", "FUSE-1"));
        memory.insert(class_of(b), PartKey::new("Acme", "FUSE-1"));
        let rows = pass_rows(project, &catalog, &PartsPass::default(), &memory);
        let chosen = |id| {
            let r = rows.iter().find(|r| r.device == id).unwrap();
            r.options[r.chosen.unwrap()].1.clone()
        };
        assert_eq!(
            chosen(b),
            "Acme MCB-1",
            "a breaker never gets the fuse link"
        );
        assert_eq!(chosen(f), "Acme FUSE-1");
    }

    fn drive_blank() -> (tempfile::TempDir, Drive) {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, crate::app::Action::NewProject);
        d.frame();
        (dir, d)
    }
}
