//! Vector PDF export (owned by WS-09): one PDF page per project page, with
//! bookmarks, page labels and clickable cross-reference links.
//!
//! A generated page (a [`PageKind::Generated`] record: bill of materials,
//! terminal diagram, ...) prints as one PDF page per sheet of its report,
//! each named like the sheet (`20`, or `20a`, `20b`, ...; see
//! [`novo_reports::sheet_names`]) in the page labels and bookmarks, with
//! the report's table drawn inside the page frame and the sheet's name and
//! "n of m" (counted within the generated page, DESIGN.md §8) in its title
//! block, through the same [`PageScene::set_sheet`] the canvas shows.
//!
//! The PDF is drawn from the same [`PageScene`] the canvas shows, so what
//! prints is exactly what is on screen (symbols, wires, junction glyphs,
//! tags, wire numbers, texts, the page frame and title block), restyled
//! black on white by [`print_style`]. The project is only read; nothing in
//! the model changes. Exports are never blocked by problems: whatever the
//! project holds is drawn.
//!
//! ```no_run
//! # fn demo(project: &novo_core::Project) -> Result<(), novo_pdf::PdfError> {
//! let bytes = novo_pdf::export_pdf(project, &novo_pdf::PdfOptions::default())?;
//! std::fs::write("project.pdf", bytes)?;
//! # Ok(()) }
//! ```
//!
//! Output is deterministic: the same project and options give the same
//! bytes (no dates, no random file id), so exports can be compared and
//! cached.

mod draw;
mod font;
mod inter;
pub mod plate;

use novo_canvas::model::{
    ModelStyle, PageScene, PlaceholderArt, SheetContent, SheetLine, SheetText, SymbolArt,
    TitleRevisions,
};
use novo_canvas::{Anchor, Color, ItemId, Primitive, Shape, Stroke, Vec2};
use novo_core::{AnyId, Page, PageId, PageKind, Project, SymbolId, SymbolRole, TextContent};
use novo_reports::{Align, GeneratedSheet, LiveReports, PageLayout};
use pdf_writer::types::{ActionType, AnnotationType, PageMode};
use pdf_writer::{Finish, Name, Pdf, Rect, Ref, TextStr};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

pub use draw::PT_PER_MM;
pub use novo_canvas::model::{NoteAnchor, SymbolNote};

/// Which pages to export.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PageSet {
    /// Every page, in the project's page order.
    #[default]
    All,
    /// These pages, in the project's page order (not the list's), each once.
    Pages(Vec<PageId>),
}

/// How to export.
#[derive(Clone, Debug, PartialEq)]
pub struct PdfOptions {
    pub pages: PageSet,
    /// Look of the drawing. [`print_style`] by default: black on white.
    pub style: ModelStyle,
    /// One bookmark per page (`1  Main circuit`), shown when the PDF opens.
    pub bookmarks: bool,
    /// Clickable links from a device's parts to its other parts on other
    /// pages, and between interruption points.
    pub links: bool,
    /// Document title; the project name when `None`.
    pub title: Option<String>,
    /// Text next to symbols, by page: the cross references the canvas
    /// shows (novo-xref's `labels_on_page`). Empty draws none.
    pub notes: BTreeMap<PageId, Vec<SymbolNote>>,
    /// The revision row of each page's title block (docs/DESIGN.md §7),
    /// worked out by the caller from the issued revisions. A page without
    /// an entry prints the project's revision properties.
    pub title_revisions: BTreeMap<PageId, TitleRevisions>,
    /// Pages with changes since the latest revision: their bookmark is
    /// marked with a dot (docs/DESIGN.md §5).
    pub changed_pages: BTreeSet<PageId>,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            pages: PageSet::All,
            style: print_style(),
            bookmarks: true,
            links: true,
            title: None,
            notes: BTreeMap::new(),
            title_revisions: BTreeMap::new(),
            changed_pages: BTreeSet::new(),
        }
    }
}

/// Black on white, the way schematics are printed: everything in black,
/// the sheet left unfilled (paper white), the frame slightly heavier than
/// the canvas default so it survives photocopying.
pub fn print_style() -> ModelStyle {
    let ink = Color::BLACK;
    ModelStyle {
        symbol: Stroke::world(0.35, ink),
        wire: Stroke::world(0.35, ink),
        tag_color: ink,
        sheet_fill: None,
        frame_color: ink,
        note_color: ink,
        note_warning_color: ink,
        joint_color: ink,
        pin_joint_color: ink,
        wire_end_color: ink,
        unconnected_color: ink,
        corner_half: 0.0,
        ..ModelStyle::default()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("{0} is not a page of the project")]
    UnknownPage(PageId),
    #[error("cannot write the PDF: {0}")]
    Io(#[from] std::io::Error),
}

/// Size of the page shown when there is nothing to export (an empty
/// project or page set): A3 landscape, in mm.
const BLANK_PAGE_MM: (f64, f64) = (420.0, 297.0);

/// Exports the project to PDF bytes, drawing symbols with placeholder art.
/// Use [`export_pdf_with_art`] to pass the symbol library.
pub fn export_pdf(project: &Project, options: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    export_pdf_with_art(project, options, &PlaceholderArt)
}

/// Exports and writes the PDF to `path`.
pub fn write_pdf(
    project: &Project,
    options: &PdfOptions,
    art: &dyn SymbolArt,
    path: impl AsRef<Path>,
) -> Result<(), PdfError> {
    let bytes = export_pdf_with_art(project, options, art)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Exports the project to PDF bytes with the given symbol artwork.
///
/// A project or page set without pages gives a one-page blank A3 PDF, so
/// the result always opens in a reader. Generated pages are filled from a
/// fresh [`LiveReports::standard`]; use [`export_pdf_with_reports`] to
/// share the app's live reports instead.
pub fn export_pdf_with_art(
    project: &Project,
    options: &PdfOptions,
    art: &dyn SymbolArt,
) -> Result<Vec<u8>, PdfError> {
    export_pdf_with_reports(project, options, art, &mut LiveReports::standard())
}

/// One printed PDF page: a project page, or one sheet of a generated page.
struct Sheet<'a> {
    page: &'a Page,
    /// Page label and bookmark name: the page name, or the sheet name.
    name: String,
    /// The report sheet drawn inside the frame of a generated page.
    generated: Option<GeneratedSheet>,
    /// "n of m" within a generated page: `(1, 1)` for one that prints a
    /// single sheet (or an empty frame); `None` for schematic pages, whose
    /// title block counts schematic pages.
    number: Option<(usize, usize)>,
    /// What the sheet shows, for its bookmark: its report section's title
    /// (`=A1+CAB-X1`, `Devices without parts`), which one-table reports leave
    /// empty; empty for schematic pages.
    section: String,
}

/// The printed sheets of `page`: itself, or one per sheet of its report. A
/// generated page whose report is unknown prints its empty frame.
fn sheets_of<'a>(project: &Project, page: &'a Page, reports: &mut LiveReports) -> Vec<Sheet<'a>> {
    let generated = match page.kind {
        PageKind::Generated { .. } => reports.sheets_of_page(project, page.id),
        _ => Vec::new(),
    };
    if generated.is_empty() {
        let number = matches!(page.kind, PageKind::Generated { .. }).then_some((1, 1));
        return vec![Sheet {
            page,
            name: page.name.clone(),
            generated: None,
            number,
            section: String::new(),
        }];
    }
    let count = generated.len();
    generated
        .into_iter()
        .enumerate()
        .map(|(k, g)| {
            let section = reports
                .get(project, &g.report)
                .and_then(|o| o.sections.get(g.sheet.section))
                .map(|s| s.title.clone())
                .unwrap_or_default();
            Sheet {
                page,
                name: g.name.clone(),
                generated: Some(g),
                number: Some((k + 1, count)),
                section,
            }
        })
        .collect()
}

/// Exports the project to PDF bytes, filling generated pages from
/// `reports` (the app's one [`LiveReports`] per document, kept current
/// with every change set).
pub fn export_pdf_with_reports(
    project: &Project,
    options: &PdfOptions,
    art: &dyn SymbolArt,
    reports: &mut LiveReports,
) -> Result<Vec<u8>, PdfError> {
    let selected = select_pages(project, &options.pages)?;
    let pages: Vec<Sheet> = selected
        .iter()
        .flat_map(|page| sheets_of(project, page, reports))
        .collect();

    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog_id = alloc();
    let tree_id = alloc();
    let font_id = alloc();
    let info_id = alloc();
    let outline_id = alloc();

    struct Out {
        page: Ref,
        content: Ref,
        size_mm: (f32, f32),
    }
    let count = pages.len().max(1);
    let out: Vec<Out> = (0..count)
        .map(|i| {
            let (w, h) = pages
                .get(i)
                .map_or(BLANK_PAGE_MM, |p| p.page.format.size_mm());
            Out {
                page: alloc(),
                content: alloc(),
                size_mm: (w as f32, h as f32),
            }
        })
        .collect();
    // A link to a generated page lands on its first sheet.
    let mut index_of: HashMap<PageId, usize> = HashMap::new();
    for (i, sheet) in pages.iter().enumerate() {
        index_of.entry(sheet.page.id).or_insert(i);
    }

    let mut pdf = Pdf::new();

    let mut catalog = pdf.catalog(catalog_id);
    catalog.pages(tree_id);
    if options.bookmarks && !pages.is_empty() {
        catalog.outlines(outline_id);
        catalog.page_mode(PageMode::UseOutlines);
    }
    let label_ids: Vec<Ref> = pages.iter().map(|_| alloc()).collect();
    if !pages.is_empty() {
        let mut labels = catalog.page_labels();
        let mut nums = labels.nums();
        for (i, id) in label_ids.iter().enumerate() {
            nums.insert(i as i32, *id);
        }
    }
    catalog.finish();
    for (page, id) in pages.iter().zip(&label_ids) {
        pdf.indirect(*id)
            .start::<pdf_writer::writers::PageLabel>()
            .prefix(TextStr(&page.name));
    }

    pdf.pages(tree_id)
        .kids(out.iter().map(|o| o.page))
        .count(out.len() as i32);

    pdf.type1_font(font_id)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));

    let title = options
        .title
        .clone()
        .unwrap_or_else(|| project.info().name.clone());
    let mut info = pdf.document_info(info_id);
    if !title.is_empty() {
        info.title(TextStr(&title));
    }
    info.producer(TextStr("KoperCAD"));
    info.finish();

    // Embedded only when some line needs it (see `inter`).
    let mut inter = inter::Inter::new();
    let mut inter_id: Option<Ref> = None;
    // Helvetica-Bold, written only when a page uses it.
    let mut bold_id: Option<Ref> = None;
    for (i, o) in out.iter().enumerate() {
        let (w, h) = o.size_mm;
        let mut writer = draw::PageWriter::new(h, &mut inter);
        let mut annotations = Vec::new();
        if let Some(sheet) = pages.get(i) {
            let page = sheet.page;
            let mut scene = PageScene::new(project, page.id, art);
            scene.set_style(options.style, project, art);
            if let Some(notes) = options.notes.get(&page.id) {
                scene.set_notes(project, notes.iter().cloned(), art);
            }
            if let Some(revisions) = options.title_revisions.get(&page.id) {
                scene.set_revisions(project, Some(revisions.clone()));
            }
            if let Some(number) = sheet.number {
                let layout = sheet
                    .generated
                    .as_ref()
                    .and_then(|g| reports.layout_sheet(project, g))
                    .unwrap_or_default();
                scene.set_sheet(project, Some(sheet_content(&sheet.name, number, &layout)));
            }
            let s = scene.scene();
            // A report sheet shows its frame and table only: anything
            // drawn on a generated page by accident would repeat on every
            // sheet.
            let frame = scene.layers().frame;
            let mut items: Vec<_> = s
                .iter()
                .filter(|(_, it)| s.layer(it.layer).is_none_or(|l| l.visible))
                .filter(|(_, it)| sheet.generated.is_none() || it.layer == frame)
                .collect();
            // Layer order, then id: independent of how the scene stores its
            // items, so exports are reproducible.
            items.sort_by_key(|(id, it)| (s.layer(it.layer).map_or(0, |l| l.order), *id));
            for (_, item) in &items {
                for p in &item.primitives {
                    writer.primitive(p);
                }
                // Open-end rings are screen-size handles on the canvas; on
                // paper they print at the style's world radius.
                for h in item.handles.iter().filter(|h| h.ring) {
                    writer.primitive(&Primitive::stroked(
                        Shape::circle(h.at, options.style.wire_end_radius),
                        Stroke::world(0.3, h.color),
                    ));
                }
                for l in &item.labels {
                    writer.label(l);
                }
            }
            if options.links {
                for (item, _) in &items {
                    let Some(target) = link_target(project, &scene, *item, &index_of) else {
                        continue;
                    };
                    let Some(b) = s.item_bounds(*item).filter(|b| !b.is_empty()) else {
                        continue;
                    };
                    let (ti, to) = target;
                    let id = alloc();
                    let th = out[ti].size_mm.1;
                    let mut a = pdf.annotation(id);
                    a.subtype(AnnotationType::Link)
                        .rect(Rect::new(
                            b.min.x * PT_PER_MM,
                            (h - b.max.y) * PT_PER_MM,
                            b.max.x * PT_PER_MM,
                            (h - b.min.y) * PT_PER_MM,
                        ))
                        .border(0.0, 0.0, 0.0, None)
                        .contents(TextStr(&format!("Go to page {}", pages[ti].name)));
                    a.action()
                        .action_type(ActionType::GoTo)
                        .destination()
                        .page(out[ti].page)
                        .xyz(
                            (to.0 - 40.0).max(0.0) * PT_PER_MM,
                            (th - (to.1 - 40.0).max(0.0)) * PT_PER_MM,
                            None,
                        );
                    a.finish();
                    annotations.push(id);
                }
            }
        }
        let mut page = pdf.page(o.page);
        page.parent(tree_id)
            .media_box(Rect::new(0.0, 0.0, w * PT_PER_MM, h * PT_PER_MM))
            .contents(o.content);
        let mut resources = page.resources();
        let mut fonts = resources.fonts();
        fonts.pair(Name(font::FONT_NAME), font_id);
        if writer.uses_inter() {
            let id = *inter_id.get_or_insert_with(&mut alloc);
            fonts.pair(Name(inter::FONT_NAME), id);
        }
        if writer.uses_bold() {
            let id = *bold_id.get_or_insert_with(&mut alloc);
            fonts.pair(Name(font::BOLD_NAME), id);
        }
        fonts.finish();
        resources.finish();
        if !annotations.is_empty() {
            page.annotations(annotations);
        }
        page.finish();
        let data = writer.finish();
        pdf.stream(o.content, &data);
    }

    if let Some(id) = inter_id {
        inter.write(&mut pdf, id, &mut alloc);
    }
    if let Some(id) = bold_id {
        pdf.type1_font(id)
            .base_font(Name(b"Helvetica-Bold"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
    }

    if options.bookmarks && !pages.is_empty() {
        let items: Vec<Ref> = pages.iter().map(|_| alloc()).collect();
        pdf.outline(outline_id)
            .first(items[0])
            .last(items[items.len() - 1])
            .count(items.len() as i32);
        for (i, sheet) in pages.iter().enumerate() {
            let mut item = pdf.outline_item(items[i]);
            item.parent(outline_id)
                .title(TextStr(&bookmark_text(sheet, options)));
            if i > 0 {
                item.prev(items[i - 1]);
            }
            if i + 1 < items.len() {
                item.next(items[i + 1]);
            }
            item.dest().page(out[i].page).fit();
        }
    }

    Ok(pdf.finish())
}

/// The pages to export, in project order.
fn select_pages<'a>(project: &'a Project, set: &PageSet) -> Result<Vec<&'a Page>, PdfError> {
    let all = project.pages_in_order();
    match set {
        PageSet::All => Ok(all),
        PageSet::Pages(ids) => {
            if let Some(missing) = ids.iter().find(|id| project.page(**id).is_none()) {
                return Err(PdfError::UnknownPage(*missing));
            }
            let wanted: BTreeSet<PageId> = ids.iter().copied().collect();
            Ok(all.into_iter().filter(|p| wanted.contains(&p.id)).collect())
        }
    }
}

/// One sheet of a generated page as the canvas draws it inside the page
/// frame: the report's table, the sheet `name` and `number` ("n of m"
/// within its page) for the title block. Screen and PDF share this, so a
/// sheet prints as it shows.
pub fn sheet_content(name: &str, number: (usize, usize), layout: &PageLayout) -> SheetContent {
    let at = |x: f64, y: f64| Vec2::new(x as f32, y as f32);
    SheetContent {
        name: name.to_owned(),
        number: Some(number),
        texts: layout
            .texts
            .iter()
            .map(|t| SheetText {
                min: at(t.x, t.y),
                size: at(t.width, t.height),
                // A table cell never wraps.
                text: t.text.lines().next().unwrap_or_default().to_owned(),
                height: t.text_mm as f32,
                align: match t.align {
                    Align::Left => Anchor::CenterLeft,
                    Align::Center => Anchor::Center,
                    Align::Right => Anchor::CenterRight,
                },
                bold: t.bold,
            })
            .collect(),
        lines: layout
            .lines
            .iter()
            .map(|l| SheetLine {
                from: at(l.from.0, l.from.1),
                to: at(l.to.0, l.to.1),
                heavy: l.heavy,
            })
            .collect(),
    }
}

/// `20b  Terminal diagram · =A1+CAB-X1`: the sheet name, the page title,
/// then what the sheet shows, so a reflowed sheet is found by name
/// (docs/DESIGN.md, generated pages under pinned flow).
/// The bookmark of a sheet: its title, with a dot in front when its page
/// has changes since the latest revision.
fn bookmark_text(sheet: &Sheet, options: &PdfOptions) -> String {
    let title = sheet_bookmark(sheet);
    if options.changed_pages.contains(&sheet.page.id) {
        format!("● {title}")
    } else {
        title
    }
}

fn sheet_bookmark(sheet: &Sheet) -> String {
    let mut title = sheet.name.clone();
    let what: Vec<&str> = [sheet.page.title.as_str(), sheet.section.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    if !what.is_empty() {
        title.push_str("  ");
        title.push_str(&what.join(" · "));
    }
    title
}

/// `1  Main circuit`, or just the name for an untitled page.
pub fn bookmark_title(page: &Page) -> String {
    if page.title.is_empty() {
        page.name.clone()
    } else {
        format!("{}  {}", page.name, page.title)
    }
}

/// Where a click on a canvas item should go: the index of the target page
/// in the export and the point on it (mm).
///
/// A device's parts link to each other across pages: a contact to its main
/// symbol (the coil), the main symbol to its first part on another page.
/// Interruption points link to their first partner on another page. A
/// device tag text links like the symbol it belongs to. Targets outside the
/// exported pages are left out.
///
/// TODO(WS-10): once cross-reference labels (`2.4` under a contact, the
/// contact image under a coil) are drawn, link each reference entry to its
/// own target instead of the whole symbol to the first one.
fn link_target(
    project: &Project,
    scene: &PageScene,
    item: ItemId,
    index_of: &HashMap<PageId, usize>,
) -> Option<(usize, (f32, f32))> {
    let symbol = match scene.model_id(item)? {
        AnyId::Symbol(s) => s,
        AnyId::Text(t) => {
            let text = project.text(t)?;
            match (text.attached_to?, &text.content) {
                (AnyId::Symbol(s), TextContent::DeviceTag) => s,
                _ => return None,
            }
        }
        _ => return None,
    };
    let here = project.symbol(symbol)?.page;
    let target = symbol_target(project, symbol, here, index_of)?;
    let sym = project.symbol(target)?;
    let index = *index_of.get(&sym.page)?;
    let o = sym.placement.origin;
    Some((index, (o.x as f32, o.y as f32)))
}

fn symbol_target(
    project: &Project,
    symbol: SymbolId,
    here: PageId,
    index_of: &HashMap<PageId, usize>,
) -> Option<SymbolId> {
    let elsewhere = |page: PageId| page != here && index_of.contains_key(&page);
    match &project.symbol(symbol)?.role {
        SymbolRole::Device { device, main } => project
            .cross_references(*device)
            .into_iter()
            .find(|r| r.main != *main && elsewhere(r.location.page))
            .map(|r| r.symbol),
        SymbolRole::Interruption { .. } => project
            .interruption_partners(symbol)
            .into_iter()
            .find(|r| elsewhere(r.location.page))
            .map(|r| r.symbol),
        _ => None,
    }
}
