//! The part picker: search the catalog and give a device or a cable its
//! part. It is one list used in two places: the inspector's Part row
//! (select a device, Part > Choose part) and the cable controls of the
//! Cables drawer (Ctrl+4, pick a cable). The Parts window has its own
//! "Use for selected device" button for the part it shows.
//!
//! Each row shows how complete the part's data is (the Quality score of the
//! Parts window), so a poorly described part is visible before it is used.
//! An assignment is one undo step.

use super::{Level, NovoApp};
use eframe::egui;
use novo_core::{AnyId, DeviceId, Project};
use novo_parts::{
    AssignMode, Catalog, CatalogPart, Grade, PartKey, assess, assign_part, suggest_for_device,
};

/// How many rows the list shows; the search narrows it.
const LIMIT: usize = 30;

/// The catalog the pickers search, loaded the first time one opens.
#[derive(Default)]
pub(super) struct PickerCatalog(Option<Catalog>);

impl PickerCatalog {
    /// The catalog, loaded on first use. A catalog file that cannot be read
    /// falls back to the starter set; the reason is returned once.
    pub(super) fn ensure(&mut self) -> Option<String> {
        if self.0.is_some() {
            return None;
        }
        match Catalog::load_default() {
            Ok(catalog) => {
                self.0 = Some(catalog);
                None
            }
            Err(e) => {
                self.0 = Some(Catalog::starter());
                Some(format!("Parts catalog: {e}"))
            }
        }
    }

    pub(super) fn get(&self) -> Option<&Catalog> {
        self.0.as_ref()
    }
}

/// Whether a picker is showing, and what is typed in its search.
#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct PartPicker {
    pub(super) open: bool,
    pub(super) query: String,
    /// The rows for the last `(scope, query)`, so a picker that stays open
    /// does not search a large catalog again every frame.
    cache: Option<(String, String, Vec<Candidate>)>,
}

impl PartPicker {
    /// Opens or closes the list; each opening searches afresh.
    pub(super) fn toggle(&mut self) {
        self.open = !self.open;
        self.cache = None;
    }

    pub(super) fn close(&mut self) {
        self.open = false;
        self.cache = None;
    }

    /// The rows for `scope` (what is being searched for: a device, the
    /// cables) and the current query. `build` runs only when either changed.
    pub(super) fn rows(
        &mut self,
        scope: &str,
        build: impl FnOnce(&str) -> Vec<Candidate>,
    ) -> Vec<Candidate> {
        if let Some((s, q, rows)) = &self.cache
            && s == scope
            && *q == self.query
        {
            return rows.clone();
        }
        let rows = build(&self.query);
        self.cache = Some((scope.to_owned(), self.query.clone(), rows.clone()));
        rows
    }
}

/// One row of the list.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Candidate {
    pub(super) key: PartKey,
    /// `Manufacturer order-number`.
    pub(super) title: String,
    pub(super) detail: String,
    pub(super) percent: u8,
    pub(super) grade: Grade,
}

impl Candidate {
    fn of(part: &CatalogPart) -> Self {
        let quality = assess(part);
        let detail = if part.description.is_empty() {
            part.type_number.clone()
        } else {
            part.description.clone()
        };
        Candidate {
            key: part.key(),
            title: format!("{} {}", part.manufacturer, part.order_number),
            detail,
            percent: quality.percent,
            grade: quality.grade(),
        }
    }
}

/// Parts that suit a device: its letter code first, then whatever the
/// search finds.
pub(super) fn device_candidates(
    catalog: &Catalog,
    project: &Project,
    device: DeviceId,
    query: &str,
) -> Vec<Candidate> {
    suggest_for_device(catalog, project, device, query, LIMIT)
        .into_iter()
        .map(Candidate::of)
        .collect()
}

/// Cable parts: the catalog parts that list cores, narrowed by the search.
pub(super) fn cable_candidates(catalog: &Catalog, query: &str) -> Vec<Candidate> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    catalog
        .parts()
        .iter()
        .filter(|p| !p.cores().is_empty())
        .filter(|p| {
            let text = format!(
                "{} {} {} {} {}",
                p.manufacturer,
                p.order_number,
                p.type_number,
                p.description,
                p.keywords.join(" ")
            )
            .to_lowercase();
            words.iter().all(|w| text.contains(w))
        })
        .take(LIMIT)
        .map(Candidate::of)
        .collect()
}

fn grade_color(grade: Grade) -> egui::Color32 {
    match grade {
        Grade::Good => novo_theme::tokens::SUCCESS,
        Grade::Fair => novo_theme::tokens::WARNING,
        Grade::Poor => novo_theme::tokens::ERROR,
    }
}

/// The search field and the list. Returns the part the user clicked.
pub(super) fn picker_ui(
    ui: &mut egui::Ui,
    picker: &mut PartPicker,
    candidates: &[Candidate],
    empty_hint: &str,
) -> Option<PartKey> {
    let mut chosen = None;
    ui.add(
        egui::TextEdit::singleline(&mut picker.query)
            .hint_text("Search order number, type or description")
            .desired_width(f32::INFINITY),
    );
    if candidates.is_empty() {
        ui.weak(empty_hint);
    }
    egui::ScrollArea::vertical()
        .id_salt("part_picker_list")
        .max_height(220.0)
        .show(ui, |ui| {
            for c in candidates {
                let response = ui
                    .selectable_label(false, &c.title)
                    .on_hover_text("Use this part");
                ui.horizontal(|ui| {
                    ui.small(
                        egui::RichText::new(format!("{} {}%", c.grade.label(), c.percent))
                            .color(grade_color(c.grade)),
                    )
                    .on_hover_text("How complete the part's data is (Parts > Quality)");
                    if !c.detail.is_empty() {
                        ui.small(&c.detail);
                    }
                });
                if response.clicked() {
                    chosen = Some(c.key.clone());
                }
            }
        });
    chosen
}

/// The command that gives `device` the part, replacing its part list. The
/// part is copied into the project once. `None` when nothing would change.
pub(super) fn assign_device_command(
    project: &Project,
    catalog: &Catalog,
    device: DeviceId,
    key: &PartKey,
) -> Option<novo_core::Command> {
    let part = catalog.get(key)?;
    assign_part(project, &[device], part, AssignMode::Replace)
}

/// The command that gives every device of `devices` the catalog part `key`,
/// as one undo step. `None` when the part is unknown or nothing changes.
pub(super) fn assign_devices_command(
    project: &Project,
    catalog: &Catalog,
    devices: &[DeviceId],
    key: &PartKey,
) -> Option<novo_core::Command> {
    let part = catalog.get(key)?;
    assign_part(project, devices, part, AssignMode::Replace)
}

/// The devices a selection draws: devices themselves and the devices of
/// selected symbols, each once.
pub(super) fn selected_devices(project: &Project, selection: &[AnyId]) -> Vec<DeviceId> {
    let mut devices = Vec::new();
    for id in selection {
        let device = match id {
            AnyId::Device(d) => Some(*d),
            AnyId::Symbol(s) => project.symbol(*s).and_then(|s| s.device()),
            _ => None,
        };
        if let Some(d) = device
            && !devices.contains(&d)
        {
            devices.push(d);
        }
    }
    devices
}

impl NovoApp {
    /// The Parts window's "Use for selected device": the part goes to every
    /// selected device as one undo step.
    pub(super) fn use_part_for_selection(&mut self, part: &CatalogPart) {
        let selection = self.selection();
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            self.message(Level::Info, "Open a project to assign parts");
            return;
        };
        let devices = selected_devices(project, &selection);
        if devices.is_empty() {
            self.message(Level::Info, "Select a device on the drawing first");
            return;
        }
        let title = format!("{} {}", part.manufacturer, part.order_number);
        let Some(command) = assign_part(project, &devices, part, AssignMode::Replace) else {
            self.message(Level::Info, format!("{title} is already the part"));
            return;
        };
        match self.execute(command) {
            Some(Ok(_)) => self.message(
                Level::Info,
                format!("{title} set for {} selected", devices.len()),
            ),
            Some(Err(e)) => self.message(Level::Error, format!("Could not set the part: {e}")),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{Document, Project};

    fn project_with_contactor() -> (Project, DeviceId) {
        let mut project = Project::new("Plant");
        let (device, _) = project
            .transact("Add", |tx| {
                tx.add_device(novo_core::DeviceTag::parse("-K1").unwrap(), "Pump")
            })
            .unwrap();
        (project, device)
    }

    #[test]
    fn a_device_gets_parts_for_its_letter_code_with_a_quality_score() {
        let (project, device) = project_with_contactor();
        let catalog = Catalog::starter();
        let rows = device_candidates(&catalog, &project, device, "");
        assert!(!rows.is_empty(), "the starter catalog has contactors");
        let fits = |c: &Candidate| catalog.get(&c.key).is_some_and(|p| p.fits("K"));
        assert!(rows.iter().all(fits));
        assert!(rows.iter().all(|c| c.percent <= 100 && !c.title.is_empty()));
    }

    #[test]
    fn a_large_catalog_is_searched_once_per_query_not_every_frame() {
        let mut catalog = Catalog::new();
        for i in 0..50_000 {
            let mut part = CatalogPart::new("Acme", format!("K-{i:05}"));
            part.description = format!("Contactor {i}");
            part.letter_codes = vec!["K".into()];
            catalog.upsert(part);
        }
        let (project, device) = project_with_contactor();
        let mut picker = PartPicker {
            open: true,
            ..PartPicker::default()
        };
        let searches = std::cell::Cell::new(0);
        let frame = |picker: &mut PartPicker| {
            picker.rows("device", |query| {
                searches.set(searches.get() + 1);
                device_candidates(&catalog, &project, device, query)
            })
        };
        let first = frame(&mut picker);
        assert_eq!(first.len(), LIMIT, "the list stays short on a big catalog");
        for _ in 0..100 {
            assert_eq!(frame(&mut picker), first);
        }
        assert_eq!(searches.get(), 1, "100 more frames reuse the rows");
        picker.query = "Contactor 49999".into();
        let narrowed = frame(&mut picker);
        assert_eq!(searches.get(), 2, "a new query searches again");
        assert!(narrowed.iter().any(|c| c.title.contains("K-49999")));
        picker.toggle();
        picker.toggle();
        frame(&mut picker);
        assert_eq!(searches.get(), 3, "reopening searches afresh");
    }

    #[test]
    fn assigning_a_part_is_one_undo_step() {
        let (project, device) = project_with_contactor();
        let catalog = Catalog::starter();
        let key = device_candidates(&catalog, &project, device, "")[0]
            .key
            .clone();
        let mut doc = Document::new(project);
        let depth = doc.undo_depth();
        let command = assign_device_command(doc.project(), &catalog, device, &key).unwrap();
        doc.execute(command).unwrap();
        assert_eq!(doc.undo_depth(), depth + 1);
        let part = doc.project().device(device).unwrap().parts[0];
        assert_eq!(
            doc.project().part(part).unwrap().order_number,
            catalog.get(&key).unwrap().order_number
        );
        // The same part again changes nothing.
        assert!(assign_device_command(doc.project(), &catalog, device, &key).is_none());
        doc.undo().unwrap();
        assert!(doc.project().device(device).unwrap().parts.is_empty());
    }

    #[test]
    fn selected_devices_come_once_from_devices_and_symbols_alike() {
        let (project, device) = project_with_contactor();
        let both = [AnyId::Device(device), AnyId::Device(device)];
        assert_eq!(selected_devices(&project, &both), vec![device]);
        assert!(selected_devices(&project, &[]).is_empty());
    }

    #[test]
    fn cable_candidates_are_the_parts_with_cores() {
        let mut catalog = Catalog::starter();
        let mut cable = CatalogPart::new("Lapp", "1119304");
        cable.description = "OLFLEX CLASSIC 110 4G1.5".into();
        cable.data.insert(
            novo_parts::keys::CORES.into(),
            novo_core::Value::Text("BN,BK,GY,GNYE".into()),
        );
        catalog.upsert(cable);
        let rows = cable_candidates(&catalog, "");
        assert!(rows.iter().any(|c| c.title == "Lapp 1119304"));
        assert!(
            rows.iter()
                .all(|c| !catalog.get(&c.key).unwrap().cores().is_empty())
        );
        assert_eq!(cable_candidates(&catalog, "olflex 4g1.5").len(), 1);
        let narrowed = cable_candidates(&catalog, "zzz-no-such-cable");
        assert!(narrowed.is_empty());
    }
}
