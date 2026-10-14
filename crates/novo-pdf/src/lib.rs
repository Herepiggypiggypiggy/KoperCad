//! Vector PDF export (owned by WS-09): one PDF page per project page, with
//! bookmarks, page labels and clickable cross-reference links.
//!
//! A generated page (a [`PageKind::Generated`] record: bill of materials,
//! terminal diagram, ...) prints as one PDF page per sheet of its report,
//! each named like the sheet (`20`, or `20a`, `20b`, ...; see
//! [`novo_reports::sheet_names`]) in the page labels and bookmarks, with
//! the report's table drawn inside the page frame.
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

use novo_canvas::model::{ModelStyle, PageScene, PlaceholderArt, SymbolArt};
use novo_canvas::{Color, ItemId, Stroke};
use novo_core::{AnyId, Page, PageId, PageKind, Project, SymbolId, SymbolRole, TextContent};
use novo_reports::{GeneratedSheet, LiveReports, PageLayout};
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
        return vec![Sheet {
            page,
            name: page.name.clone(),
            generated: None,
            section: String::new(),
        }];
    }
    generated
        .into_iter()
        .map(|g| {
            let section = reports
                .get(project, &g.report)
                .and_then(|o| o.sections.get(g.sheet.section))
                .map(|s| s.title.clone())
                .unwrap_or_default();
            Sheet {
                page,
                name: g.name.clone(),
                generated: Some(g),
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
    info.producer(TextStr("NovoElectric"));
    info.finish();

    for (i, o) in out.iter().enumerate() {
        let (w, h) = o.size_mm;
        let mut writer = draw::PageWriter::new(h);
        let mut annotations = Vec::new();
        if let Some(sheet) = pages.get(i) {
            let page = sheet.page;
            let mut scene = PageScene::new(project, page.id, art);
            scene.set_style(options.style, project, art);
            if let Some(notes) = options.notes.get(&page.id) {
                scene.set_notes(project, notes.iter().cloned(), art);
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
                for l in &item.labels {
                    writer.label(l);
                }
            }
            if let Some(layout) = sheet
                .generated
                .as_ref()
                .and_then(|g| reports.layout_sheet(project, g))
            {
                draw_layout(&mut writer, &layout, options.style.frame_color);
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
        page.resources()
            .fonts()
            .pair(Name(font::FONT_NAME), font_id);
        if !annotations.is_empty() {
            page.annotations(annotations);
        }
        page.finish();
        let data = writer.finish();
        pdf.stream(o.content, &data);
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
                .title(TextStr(&sheet_bookmark(sheet)));
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

/// A report table, its rules and text boxes, in the frame colour.
fn draw_layout(writer: &mut draw::PageWriter, layout: &PageLayout, ink: Color) {
    use novo_canvas::{Primitive, Shape, Vec2};
    let heavy = Stroke::world(0.35, ink);
    let thin = Stroke::world(0.18, ink);
    for line in &layout.lines {
        let at = |(x, y): (f64, f64)| Vec2::new(x as f32, y as f32);
        let stroke = if line.heavy { heavy } else { thin };
        writer.primitive(&Primitive::stroked(
            Shape::line(at(line.from), at(line.to)),
            stroke,
        ));
    }
    for text in &layout.texts {
        writer.text_box(text, ink);
    }
}

/// `20b  Terminal diagram · =A1+CAB-X1`: the sheet name, the page title,
/// then what the sheet shows, so a reflowed sheet is found by name
/// (docs/DESIGN.md, generated pages under pinned flow).
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
