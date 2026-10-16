//! Templates that ship with the program, and the starting points of the
//! designer's "New" menu.

use novo_core::PageFormat;

use super::table::TableSpec;
use super::template::{Element, Frame, TableElement, Template, TemplateKind, TitleBlock};

/// A frame and the standard title block on `format`, nothing else.
pub fn sheet(name: &str, format: PageFormat) -> Template {
    let mut t = Template::new(name, TemplateKind::Sheet, format);
    t.frame = Some(Frame::default());
    t.title_block = Some(TitleBlock::default());
    t
}

/// An A4 landscape report: frame, title block and one table of `report`
/// (a report id) filling the area above the title block, headed by the
/// name.
pub fn report_a4(name: &str, report: &str) -> Template {
    let mut t = sheet(name, Template::a4_landscape());
    t.kind = TemplateKind::Report;
    let (ix, iy, iw, _) = t.inner_rect();
    let block_top = t.title_block_rect().map_or(iy, |b| b.1);
    let mut spec = TableSpec::new(report);
    spec.heading = name.to_owned();
    t.elements.push(Element::Table(TableElement {
        x: ix + 5.0,
        y: iy + 5.0,
        width: iw - 10.0,
        height: block_top - 4.0 - (iy + 5.0),
        spec,
    }));
    t
}

/// A4 portrait cover sheet: project name, description, revision.
pub fn cover_form() -> Template {
    let mut t = Template::new(
        "Cover sheet",
        TemplateKind::Form,
        PageFormat {
            size: novo_core::PaperSize::A4,
            landscape: false,
            ..PageFormat::default()
        },
    );
    t.frame = Some(Frame {
        margin_mm: 10.0,
        column_numbers: false,
    });
    let mut title = Element::text(25.0, 60.0, 160.0, 20.0, "{project.name}", 9.0);
    if let Element::Text { bold, .. } = &mut title {
        *bold = true;
    }
    t.elements.push(title);
    t.elements.push(Element::text(
        25.0,
        82.0,
        160.0,
        10.0,
        "{project.description}",
        4.5,
    ));
    t.elements.push(Element::text(
        25.0,
        250.0,
        160.0,
        8.0,
        "Revision {revision}  {revision_date}",
        3.5,
    ));
    t
}

/// The shipped templates.
pub fn all() -> Vec<Template> {
    vec![
        sheet("Sheet A3", PageFormat::default()),
        report_a4("Parts list", "bom"),
        report_a4("Device list", "devices"),
        report_a4("Wire list", "wires"),
        cover_form(),
    ]
}
