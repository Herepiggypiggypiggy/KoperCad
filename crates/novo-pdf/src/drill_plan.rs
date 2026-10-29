//! The hand drilling plan as a PDF: A4 portrait pages with the datum, the
//! tool list and the check-off table (`novo_layout::drill_plan`). The pages
//! are plain marks, drawn with the plate drawing's renderer at 1:1.

use novo_canvas::Vec2;
use novo_layout::drill_plan::{DrillPlan, PAGE_MM};
use pdf_writer::{Finish, Name, Pdf, Rect, Ref, TextStr};

use crate::draw::{PT_PER_MM, PageWriter};
use crate::plate::render;
use crate::{font, inter};

/// The drill plan as one PDF, one page after the other. Deterministic. A
/// plan with no sheets gives a PDF with no pages' worth of work: callers
/// check [`DrillPlan::sheets`] first.
pub fn export_drill_plan_pdf(plan: &DrillPlan) -> Vec<u8> {
    let pages = plan.pages();
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
    let ids: Vec<(Ref, Ref)> = pages.iter().map(|_| (alloc(), alloc())).collect();

    let mut pdf = Pdf::new();
    pdf.catalog(catalog_id).pages(tree_id);
    pdf.pages(tree_id)
        .kids(ids.iter().map(|i| i.0))
        .count(ids.len() as i32);
    pdf.type1_font(font_id)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    let mut info = pdf.document_info(info_id);
    info.title(TextStr(&format!("{} - drill plan", plan.project)));
    info.producer(TextStr("KoperCAD"));
    info.finish();

    let mut inter = inter::Inter::new();
    let mut inter_id: Option<Ref> = None;
    let mut bold_id: Option<Ref> = None;
    let (pw, ph) = (PAGE_MM.0 as f32, PAGE_MM.1 as f32);
    for (i, page) in pages.iter().enumerate() {
        let (page_id, content_id) = ids[i];
        let mut w = PageWriter::new(ph, &mut inter);
        let identity = |p: (f64, f64)| Vec2::new(p.0 as f32, p.1 as f32);
        render(&mut w, &page.marks, &identity, 1.0);

        let mut p = pdf.page(page_id);
        p.parent(tree_id)
            .media_box(Rect::new(0.0, 0.0, pw * PT_PER_MM, ph * PT_PER_MM))
            .contents(content_id);
        let mut resources = p.resources();
        let mut fonts = resources.fonts();
        fonts.pair(Name(font::FONT_NAME), font_id);
        if w.uses_inter() {
            let id = *inter_id.get_or_insert_with(&mut alloc);
            fonts.pair(Name(inter::FONT_NAME), id);
        }
        if w.uses_bold() {
            let id = *bold_id.get_or_insert_with(&mut alloc);
            fonts.pair(Name(font::BOLD_NAME), id);
        }
        fonts.finish();
        resources.finish();
        p.finish();
        pdf.stream(content_id, &w.finish());
    }
    if let Some(id) = inter_id {
        inter.write(&mut pdf, id, &mut alloc);
    }
    if let Some(id) = bold_id {
        pdf.type1_font(id)
            .base_font(Name(b"Helvetica-Bold"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
    }
    pdf.finish()
}
