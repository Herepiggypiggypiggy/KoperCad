//! "Add CAD files" in the Parts window: any number of downloaded STEP, DXF,
//! ZIP or EDZ files, each matched to the parts it is for, with the reasons
//! shown before anything is stored.
//!
//! A maker's download is often not named after the article, and the old
//! window filed a dropped file under whatever part was selected. Here every
//! file is read (its name, the names inside it, its size) and checked by
//! `novo_parts::cadmatch`; the user sees a verdict and the evidence, ticks the
//! parts, and presses Import. A file that fits several parts (a family model
//! such as `LC1D09-18`) can be stored for all of them. Design: the
//! "CAD file matching" section of `docs/PARTS-LIBRARY-DESIGN.md`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use eframe::egui;
use novo_parts::{
    CadIdentity, CadLinks, Candidate, Catalog, CatalogPart, FileKind, ImportFlow, LocalLibrary,
    Matcher, SizeMeasure, SourceTable, Verdict, assess_part, match_batch, step_box,
};

use super::parts_import::{Checked, import_checked};

/// Files larger than this are matched by their name only: reading them
/// whole would freeze the window.
const MAX_READ_BYTES: u64 = 64 * 1024 * 1024;

/// What a dropped or picked file is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dropped {
    /// A model or drawing for a part (STEP, DXF, EDZ, or a ZIP holding them).
    Cad,
    /// A catalog file (BMEcat, CSV, a ZIP without a model).
    Catalog,
    /// An Excel table.
    Table,
    /// A library bundle written by Export library.
    Bundle,
    Other,
}

pub(crate) fn classify(path: &Path) -> Dropped {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    if name.to_ascii_lowercase().ends_with(".xlsx") {
        return Dropped::Table;
    }
    if name.to_ascii_lowercase().ends_with(".kcpl") {
        return Dropped::Bundle;
    }
    match FileKind::of_file_name(&name) {
        FileKind::Step | FileKind::Dxf | FileKind::Edz => Dropped::Cad,
        FileKind::Zip => {
            if identity_of(path).holds_model {
                Dropped::Cad
            } else {
                Dropped::Catalog
            }
        }
        FileKind::Bmecat | FileKind::Csv => Dropped::Catalog,
        FileKind::Other => Dropped::Other,
    }
}

pub(crate) use super::parts_identity::identity_of;

/// Reads the file at `path` for what it says about itself. Never fails: an
/// unreadable or very large file just says less. Slow for a large STEP file:
/// the window goes through `parts_identity`, which reads on a worker thread
/// and remembers the answer.
pub(crate) fn read_identity(path: &Path) -> CadIdentity {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let small = std::fs::metadata(path).is_ok_and(|m| m.len() <= MAX_READ_BYTES);
    let Some(bytes) = small.then(|| std::fs::read(path).ok()).flatten() else {
        return CadIdentity::from_file_name(&name);
    };
    let mut id = CadIdentity::read(&name, &bytes);
    if id.kind == FileKind::Step {
        let text = String::from_utf8_lossy(&bytes);
        // The real solid when it can be measured; else the box around all
        // points, which can only contradict a part, never confirm it.
        let measured = novo_interop::part3d::measure_step(&text)
            .ok()
            .or_else(|| step_box(&text).ok());
        if let Some(b) = measured {
            id = id.with_size(SizeMeasure {
                size_mm: b.size_mm,
                exact: !b.approximate(),
            });
        }
    }
    id
}

struct Row {
    path: PathBuf,
    identity: CadIdentity,
    candidates: Vec<Candidate>,
    ticked: Vec<bool>,
    /// Said above the candidates (why the file was asked about).
    notes: Vec<String>,
    /// Text of the "add another part" search.
    pick: String,
    flow: ImportFlow,
}

enum Event {
    Line(String),
    Changed(Vec<CatalogPart>),
    Finished,
}

struct Job {
    rx: Receiver<Event>,
    total: usize,
    done: usize,
}

/// The open panel.
pub(crate) struct CadPanel {
    rows: Vec<Row>,
    links: CadLinks,
    job: Option<Job>,
    /// One line per stored file and part, and why a file failed.
    pub(crate) results: Vec<String>,
}

pub(crate) enum CadAction {
    Import,
    Close,
}

/// Which candidates start ticked: the ones that name the order number, and a
/// sole probable part. Several probable parts (a family) are left for the
/// user to tick.
fn default_ticks(c: &[Candidate]) -> Vec<bool> {
    let probable = c.iter().filter(|x| x.verdict >= Verdict::Likely).count();
    c.iter()
        .map(|x| x.verdict == Verdict::Verified || (x.verdict == Verdict::Likely && probable == 1))
        .collect()
}

impl CadPanel {
    /// Reads and matches `paths`.
    pub(crate) fn new(
        paths: &[PathBuf],
        catalog: &Catalog,
        library_root: Option<&Path>,
        flow: ImportFlow,
    ) -> Self {
        let links = library_root.map_or_else(CadLinks::default, CadLinks::load);
        let ids: Vec<CadIdentity> = paths.iter().map(|p| identity_of(p)).collect();
        let matched = match_batch(ids, catalog, &links, 8);
        let rows = paths
            .iter()
            .zip(matched)
            .map(|(path, m)| Row {
                path: path.clone(),
                ticked: default_ticks(&m.candidates),
                identity: m.identity,
                candidates: m.candidates,
                notes: Vec::new(),
                pick: String::new(),
                flow,
            })
            .collect();
        CadPanel {
            rows,
            links,
            job: None,
            results: Vec::new(),
        }
    }

    /// A file dropped on `target` that looks like it is for something else:
    /// the part it was dropped on first (unticked), the better home ticked,
    /// and the reasons above.
    pub(crate) fn for_doubt(
        path: &Path,
        target: &CatalogPart,
        reasons: Vec<String>,
        better: Option<Candidate>,
        catalog: &Catalog,
        library_root: Option<&Path>,
        flow: ImportFlow,
    ) -> Self {
        let links = library_root.map_or_else(CadLinks::default, CadLinks::load);
        let identity = identity_of(path);
        let dropped_on = assess_part(&identity, target, Some(catalog), &links);
        let mut candidates = vec![dropped_on];
        let mut ticked = vec![false];
        if let Some(b) = better {
            candidates.push(b);
            ticked.push(true);
        }
        CadPanel {
            rows: vec![Row {
                path: path.to_path_buf(),
                identity,
                candidates,
                ticked,
                notes: reasons,
                pick: String::new(),
                flow,
            }],
            links,
            job: None,
            results: Vec::new(),
        }
    }

    pub(crate) fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Whether any file has a part ticked.
    pub(crate) fn can_import(&self) -> bool {
        !self.busy() && self.rows.iter().any(|r| r.ticked.iter().any(|t| *t))
    }

    /// Every probable part of every file, ticked.
    fn tick_probable(&mut self) {
        for r in &mut self.rows {
            for (t, c) in r.ticked.iter_mut().zip(&r.candidates) {
                *t |= c.verdict >= Verdict::Likely;
            }
        }
    }

    /// Adds `part` to a file's candidates, ticked: the user's own choice.
    fn add_part(&mut self, row: usize, part: &CatalogPart, catalog: &Catalog) {
        let links = &self.links;
        let Some(r) = self.rows.get_mut(row) else {
            return;
        };
        if r.candidates.iter().any(|c| c.key == part.key()) {
            for (c, t) in r.candidates.iter().zip(&mut r.ticked) {
                if c.key == part.key() {
                    *t = true;
                }
            }
            return;
        }
        r.candidates
            .push(assess_part(&r.identity, part, Some(catalog), links));
        r.ticked.push(true);
    }

    /// Starts storing every ticked file for its ticked parts, on a worker
    /// thread; [`CadPanel::poll`] collects the result.
    pub(crate) fn start(
        &mut self,
        catalog: &Catalog,
        library: LocalLibrary,
        sources: SourceTable,
        catalog_path: Option<PathBuf>,
        today: String,
        wake: impl Fn() + Send + 'static,
    ) {
        struct Work {
            path: PathBuf,
            part: CatalogPart,
            flow: ImportFlow,
            checked: Checked,
        }
        let mut work = Vec::new();
        for r in &self.rows {
            for (c, ticked) in r.candidates.iter().zip(&r.ticked) {
                let Some(part) = ticked.then(|| catalog.get(&c.key)).flatten() else {
                    continue;
                };
                let mut notes = c.evidence.clone();
                notes.extend(c.warnings.clone());
                work.push(Work {
                    path: r.path.clone(),
                    part: part.clone(),
                    flow: r.flow,
                    checked: Checked {
                        notes,
                        link: c.verdict >= Verdict::Likely,
                    },
                });
            }
        }
        if work.is_empty() {
            return;
        }
        self.results.clear();
        let (tx, rx) = channel();
        let total = work.len();
        let spawned = std::thread::Builder::new()
            .name("parts-cad".into())
            .spawn(move || {
                for w in work {
                    let file = w
                        .path
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        import_checked(
                            &w.part,
                            &w.path,
                            w.flow,
                            &library,
                            &sources,
                            catalog_path.as_deref(),
                            &today,
                            &w.checked,
                        )
                    }))
                    .unwrap_or_else(|_| Err("the file reader stopped unexpectedly".into()));
                    match run {
                        Ok((summary, changed)) => {
                            let _ = tx.send(Event::Line(format!("{file} stored for {}.", w.part)));
                            for l in summary.lines {
                                let _ = tx.send(Event::Line(format!("    {l}")));
                            }
                            let _ = tx.send(Event::Changed(changed));
                        }
                        Err(e) => {
                            let _ = tx.send(Event::Line(format!(
                                "{file} could not be stored for {}: {e}",
                                w.part
                            )));
                        }
                    }
                    wake();
                }
                let _ = tx.send(Event::Finished);
                wake();
            });
        match spawned {
            Ok(_) => {
                self.job = Some(Job { rx, total, done: 0 });
            }
            Err(e) => self
                .results
                .push(format!("Could not start the import: {e}")),
        }
    }

    /// Collects what the worker has done. Returns the parts it changed, for
    /// the window to merge into its catalog.
    pub(crate) fn poll(&mut self) -> Vec<CatalogPart> {
        let mut changed = Vec::new();
        let Some(job) = self.job.as_mut() else {
            return changed;
        };
        loop {
            match job.rx.try_recv() {
                Ok(Event::Line(l)) => {
                    if !l.starts_with(' ') {
                        job.done += 1;
                    }
                    self.results.push(l);
                }
                Ok(Event::Changed(c)) => changed.extend(c),
                Ok(Event::Finished) => {
                    self.job = None;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.results.push("The import stopped unexpectedly.".into());
                    self.job = None;
                    break;
                }
            }
        }
        changed
    }

    /// Whether the import has run and the panel only shows its result.
    pub(crate) fn finished(&self) -> bool {
        self.job.is_none() && !self.results.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn tick_for_test(&mut self, row: usize, candidate: usize, on: bool) {
        self.rows[row].ticked[candidate] = on;
    }

    #[cfg(test)]
    pub(crate) fn candidates_for_test(&self, row: usize) -> Vec<(String, Verdict, bool)> {
        let r = &self.rows[row];
        r.candidates
            .iter()
            .zip(&r.ticked)
            .map(|(c, t)| (c.label.clone(), c.verdict, *t))
            .collect()
    }
}

fn chip(ui: &mut egui::Ui, verdict: Verdict) {
    let pal = crate::theme::ui_palette();
    let color = match verdict {
        Verdict::Verified => pal.success,
        Verdict::Likely => pal.text_primary,
        Verdict::Check => ui.visuals().warn_fg_color,
        Verdict::Unmatched => pal.text_tertiary,
    };
    ui.colored_label(color, verdict.label());
}

/// Draws the panel. Returns what the user asked for.
pub(crate) fn cad_ui(
    ui: &mut egui::Ui,
    panel: &mut CadPanel,
    catalog: &Catalog,
    reading: bool,
) -> Option<CadAction> {
    let mut action = None;
    let mut add: Option<(usize, CatalogPart)> = None;
    let mut tick_all = false;
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.strong("Add CAD files");
            ui.weak(
                "KoperCAD reads each file's own names and size and checks them against your parts. \
Nothing is stored until you press Import.",
            );
        });
        let busy = panel.busy();
        // Leave room for the parts list below: at most 300, at least 140
        // points, about a third of the window otherwise.
        let rows_height = (ui.ctx().content_rect().height() * 0.35).clamp(140.0, 300.0);
        egui::ScrollArea::vertical()
            .id_salt("cad_rows")
            .max_height(rows_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (ri, row) in panel.rows.iter_mut().enumerate() {
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(row.path.file_name().map_or_else(String::new, |n| {
                            n.to_string_lossy().into_owned()
                        }));
                        if let Some(best) = row.candidates.first() {
                            chip(ui, best.verdict);
                        }
                    });
                    let warn = ui.visuals().warn_fg_color;
                    for n in &row.notes {
                        ui.colored_label(warn, n);
                    }
                    if row.candidates.is_empty() {
                        ui.colored_label(
                            warn,
                            "No part in your catalog matches this file. Search for the part below.",
                        );
                    }
                    for (ci, c) in row.candidates.iter().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_enabled(
                                !busy,
                                egui::Checkbox::new(&mut row.ticked[ci], c.label.as_str()),
                            );
                            if row.candidates.len() > 1 || row.notes.is_empty() {
                                chip(ui, c.verdict);
                            }
                        });
                        ui.indent(("cad_evidence", ri, ci), |ui| {
                            for e in &c.evidence {
                                ui.weak(e);
                            }
                            for w in &c.warnings {
                                ui.colored_label(warn, w);
                            }
                            // Could not be checked, or only worth knowing:
                            // quiet, so a real warning stands out.
                            for n in &c.notes {
                                ui.weak(n);
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            !busy,
                            egui::TextEdit::singleline(&mut row.pick)
                                .hint_text("Another part: search order number or type")
                                .desired_width(280.0),
                        );
                        let m = Matcher::new(&row.pick);
                        if !m.is_empty() {
                            let hits = catalog
                                .parts()
                                .iter()
                                .filter(|p| {
                                    m.matches([
                                        p.manufacturer.as_str(),
                                        p.order_number.as_str(),
                                        p.type_number.as_str(),
                                        p.description.as_str(),
                                    ])
                                })
                                .take(4);
                            for p in hits {
                                if ui.small_button(p.to_string()).clicked() {
                                    add = Some((ri, p.clone()));
                                }
                            }
                        }
                    });
                }
            });
        if let Some(job) = &panel.job {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("Storing {} of {}\u{2026}", job.done + 1, job.total));
            });
        }
        if !panel.results.is_empty() {
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("cad_results")
                .max_height(120.0)
                .show(ui, |ui| {
                    for l in &panel.results {
                        ui.label(l);
                    }
                });
        }
        if panel.rows.iter().any(|r| r.candidates.len() > 1) {
            ui.weak(
                "A family model fits several parts: tick the ones you want, or press Tick all probable.",
            );
        }
        ui.horizontal(|ui| {
            // New files are being read: this panel is replaced when they are.
            let import = panel.can_import() && !reading;
            if ui
                .add_enabled(import, egui::Button::new("Import ticked"))
                .on_hover_text(if reading {
                    "Wait until the new files have been read."
                } else {
                    "Store each file in your parts library for the parts you ticked."
                })
                .clicked()
            {
                action = Some(CadAction::Import);
            }
            if ui
                .add_enabled(!panel.busy() && !reading, egui::Button::new("Tick all probable"))
                .on_hover_text(
                    "A family model (LC1D09-18) fits several parts; this ticks every one it probably fits.",
                )
                .clicked()
            {
                tick_all = true;
            }
            if ui
                .button(if panel.finished() { "Done" } else { "Close" })
                .clicked()
            {
                action = Some(CadAction::Close);
            }
        });
    });
    if tick_all {
        panel.tick_probable();
    }
    if let Some((row, part)) = add {
        panel.add_part(row, &part, catalog);
    }
    action
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, Instant};

    use novo_parts::Dimensions;

    use super::*;

    const STEP: &str = "ISO-10303-21;\nHEADER;\nFILE_NAME('LC1D09-18-DC-3D_ASM','2000-01-01T00:00:00',('AUTHOR01'),(\n'Schneider-Electric'),'CREO','CREO','');\nENDSEC;\nDATA;\n#9=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n#1=CARTESIAN_POINT('',(0.,0.,0.));\n#2=CARTESIAN_POINT('',(45.,77.,90.));\n#7=PRODUCT('LC1D09-18-DC-3D_ASM','LC1D09-18-DC-3D_ASM','NOT SPECIFIED',(#2));\nENDSEC;\nEND-ISO-10303-21;\n";

    fn part(order: &str) -> CatalogPart {
        CatalogPart {
            manufacturer: "Schneider Electric".into(),
            order_number: order.into(),
            description: "Contactor coil 24 V DC".into(),
            dimensions: Some(Dimensions {
                width_mm: 45.0,
                height_mm: 77.0,
                depth_mm: 86.0,
            }),
            ..Default::default()
        }
    }

    fn catalog() -> Catalog {
        let mut c = Catalog::new();
        for o in ["LC1D09BD", "LC1D12BD", "LC1D18BD", "LC1D32BD"] {
            c.upsert(part(o));
        }
        c
    }

    fn run_to_end(panel: &mut CadPanel) -> Vec<CatalogPart> {
        let started = Instant::now();
        let mut changed = Vec::new();
        while panel.busy() {
            changed.extend(panel.poll());
            assert!(started.elapsed() < Duration::from_secs(20), "stuck");
            std::thread::sleep(Duration::from_millis(10));
        }
        changed
    }

    #[test]
    fn several_files_are_matched_at_once_and_a_family_is_left_for_the_user_to_tick() {
        let dir = tempfile::tempdir().unwrap();
        let step = dir.path().join("MCADPP0003371_3D-simplified.stp");
        let dxf = dir.path().join("MCADPP0003371_2D-front.dxf");
        let named = dir.path().join("LC1D32BD.stp");
        fs::write(&step, STEP).unwrap();
        fs::write(&dxf, "0\nSECTION\n").unwrap();
        fs::write(&named, STEP.replace("LC1D09-18-DC-3D_ASM", "x")).unwrap();
        let panel = CadPanel::new(
            &[step, dxf, named],
            &catalog(),
            Some(dir.path()),
            ImportFlow::Manual,
        );
        // The family model fits three parts: none ticked by default.
        let c = panel.candidates_for_test(0);
        assert_eq!(c.len(), 3, "{c:?}");
        assert!(
            c.iter().all(|(_, v, t)| *v == Verdict::Likely && !*t),
            "{c:?}"
        );
        // The drawing has no names but arrived with it.
        assert_eq!(panel.candidates_for_test(1).len(), 3);
        // The file that names its article is checked and ticked.
        let c = panel.candidates_for_test(2);
        assert_eq!(
            c,
            [(
                "Schneider Electric LC1D32BD".into(),
                Verdict::Verified,
                true
            )]
        );
    }

    #[test]
    fn storing_a_family_file_for_two_parts_and_remembering_its_set() {
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("lib");
        let step = dir.path().join("MCADPP0003371_3D-simplified.stp");
        fs::write(&step, STEP).unwrap();
        let cat = catalog();
        let mut panel = CadPanel::new(&[step], &cat, Some(&lib), ImportFlow::Manual);
        panel.tick_for_test(0, 0, true);
        panel.tick_for_test(0, 1, true);
        assert!(panel.can_import());
        panel.start(
            &cat,
            LocalLibrary::at(&lib),
            SourceTable::builtin(),
            Some(dir.path().join("parts.json")),
            "2026-10-28".into(),
            || {},
        );
        assert!(panel.busy());
        run_to_end(&mut panel);
        let l = LocalLibrary::at(&lib);
        assert!(l.has(&part("LC1D09BD").key()) && l.has(&part("LC1D12BD").key()));
        assert!(!l.has(&part("LC1D18BD").key()), "only the ticked parts");
        let joined = panel.results.join("\n");
        assert!(
            joined.contains("stored for Schneider Electric LC1D09BD"),
            "{joined}"
        );
        assert!(joined.contains("Checked: "), "{joined}");
        assert!(joined.contains("MCADPP0003371 is remembered"), "{joined}");
        // The drawing of the same set now finds the parts without being named.
        let dxf = dir.path().join("MCADPP0003371_2D-front.dxf");
        fs::write(&dxf, "0\nSECTION\n").unwrap();
        let again = CadPanel::new(&[dxf], &cat, Some(&lib), ImportFlow::Manual);
        let c = again.candidates_for_test(0);
        assert!(
            c.iter()
                .any(|(l, v, _)| l.ends_with("LC1D09BD") && *v == Verdict::Likely),
            "{c:?}"
        );
    }

    #[test]
    fn a_wrong_drop_asks_and_nothing_is_stored_until_import() {
        let dir = tempfile::tempdir().unwrap();
        let step = dir.path().join("MCADPP0003371_3D-simplified.stp");
        fs::write(&step, STEP).unwrap();
        let mut cat = catalog();
        cat.upsert(part("LC1F185BD"));
        let target = cat.find("Schneider Electric", "LC1F185BD").unwrap().clone();
        let id = identity_of(&step);
        let novo_parts::DropDecision::Ask { reasons, better } =
            novo_parts::decide_drop(&id, &target, &cat, &CadLinks::default())
        else {
            panic!("should ask");
        };
        let panel = CadPanel::for_doubt(
            &step,
            &target,
            reasons,
            better,
            &cat,
            None,
            ImportFlow::Manual,
        );
        let c = panel.candidates_for_test(0);
        assert_eq!(c[0].0, "Schneider Electric LC1F185BD");
        assert!(!c[0].2, "the part it was dropped on is not ticked");
        assert!(c[1].2, "the better home is");
        assert!(!dir.path().join("lib").exists());
    }

    /// With `KOPERCAD_REAL_CAD_DIR` pointing at the real Schneider LC1D09
    /// download, the solid's own size is measured and confirms the part.
    #[test]
    fn the_real_schneider_model_is_measured_as_a_solid_and_its_size_fits() {
        let Some(dir) = std::env::var_os("KOPERCAD_REAL_CAD_DIR").map(PathBuf::from) else {
            return;
        };
        let step = dir.join("MCADPP0003371_3D-simplified.stp");
        if !step.exists() {
            return;
        }
        let id = identity_of(&step);
        let size = id.size.expect("a size was measured");
        assert!(
            size.exact,
            "the solid, not the box around all points: {size:?}"
        );
        let mut cat = catalog();
        cat.upsert(part("LC1D09BD"));
        let p = cat.find("Schneider Electric", "LC1D09BD").unwrap();
        let c = assess_part(&id, p, Some(&cat), &CadLinks::default());
        eprintln!("{:?} {:?}", c.verdict, c.evidence);
        assert!(
            c.evidence.iter().any(|e| e.contains("the size fits")),
            "{:#?}",
            c.evidence
        );
    }

    #[test]
    fn classify_tells_a_model_zip_from_a_catalog_zip() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let write_zip = |name: &str, member: &str| {
            let p = dir.path().join(name);
            let mut z = zip::ZipWriter::new(fs::File::create(&p).unwrap());
            z.start_file(member, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
            p
        };
        assert_eq!(classify(&write_zip("a.zip", "m.stp")), Dropped::Cad);
        assert_eq!(
            classify(&write_zip("b.zip", "catalog.xml")),
            Dropped::Catalog
        );
        assert_eq!(classify(Path::new("x.xlsx")), Dropped::Table);
        assert_eq!(classify(Path::new("x.dxf")), Dropped::Cad);
        assert_eq!(classify(Path::new("x.pdf")), Dropped::Other);
    }
}
