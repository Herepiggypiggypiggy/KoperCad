//! Writes a KoperCAD project as a PXF project file for EPLAN to import
//! (`File > Export > Export EPLAN project (PXF)`).
//!
//! What is written: sections and locations through the page and tag fields,
//! one `O4` page per page (name, title), devices and terminals as `O17`
//! objects on the page of the symbol that draws them (tag, description,
//! part number, position, and the EPLAN symbol key when the person's own
//! symbol table has one), cables as `O17` with their cores as `O59`.
//!
//! What is not written yet and is counted: wires, texts, interruption
//! points, symbols the table has no EPLAN key for (written without a symbol
//! reference), anything that is not a device, terminal or cable.
//!
//! Verified versus unverified: the field names are the ones the reader
//! reads, and a test reads every written file back. That proves the file is
//! consistent with this reader, **not** that EPLAN accepts it: no KoperCAD
//! file has been opened in a real EPLAN yet, and the report says so on every
//! export. Everything specific to one EPLAN release is in [`Dialect`].

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use novo_core::{
    DeviceId, DeviceKind, DeviceTag, Project, SymbolInstance, SymbolRole, Terminal, TerminalId,
};
use novo_symbols::Pack;

use super::dialect::{Dialect, EPLAN_2024};
use crate::eplan::symbols::{SymbolKey, SymbolMapper, UserTable};
use crate::report::Report;

/// First line after the XML declaration; the app replaces only files that
/// carry it.
pub const WRITER_MARK: &str = "Written by KoperCAD";

/// Choices for writing.
#[derive(Debug, Clone)]
pub struct PxfWriteOptions {
    pub dialect: Dialect,
    /// Language of multi-language texts (`en_US`).
    pub lang: String,
    /// The person's own EPLAN symbol keys; without one a device is written
    /// without a symbol reference.
    pub symbols: UserTable,
}

impl Default for PxfWriteOptions {
    fn default() -> Self {
        Self {
            dialect: EPLAN_2024,
            lang: EPLAN_2024.default_lang.to_string(),
            symbols: UserTable::default(),
        }
    }
}

/// The finished file and what it left out.
#[derive(Debug, Clone, PartialEq)]
pub struct PxfExport {
    pub text: String,
    pub report: Report,
}

/// Whether `head` (the start of a file) was written by this writer.
pub fn is_kopercad_pxf(head: &[u8]) -> bool {
    String::from_utf8_lossy(&head[..head.len().min(1500)]).contains(WRITER_MARK)
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A multi-language text (`en_US@text;`). A `;` ends the language entry, so
/// it is replaced; an empty text is `None` because an empty multi-language
/// text hangs EPLAN's import.
fn ml(lang: &str, text: &str) -> Option<String> {
    let clean: String = text
        .chars()
        .map(|c| match c {
            ';' => ',',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let clean = clean.trim();
    (!clean.is_empty()).then(|| format!("{lang}@{clean};"))
}

fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn levels(l: &[String]) -> String {
    l.join(".")
}

/// The tag fields of a device tag, or `None` when the product part is not a
/// single level (EPLAN has one device level).
struct TagFields {
    function: String,
    location: String,
    letters: String,
    number: String,
    sub: String,
    display: String,
}

fn tag_fields(tag: &DeviceTag) -> Option<TagFields> {
    let [code] = tag.product.as_slice() else {
        return None;
    };
    let (letters, number, suffix) = novo_core::split_code(code);
    Some(TagFields {
        function: levels(&tag.function),
        location: levels(&tag.location),
        letters: letters.to_string(),
        number: number.map(|n| n.to_string()).unwrap_or_default(),
        sub: suffix.trim_start_matches('.').to_string(),
        display: tag.to_string(),
    })
}

struct Writer {
    out: String,
    next: u64,
}

impl Writer {
    fn id(&mut self, kind: u32) -> String {
        let n = self.next;
        self.next += 1;
        format!("{kind}/{n}")
    }

    fn tag(&mut self, f: &TagFields) {
        let _ = writeln!(
            self.out,
            "      <P150 P1100=\"{}\" P1200=\"{}\" P20012=\"-\" P20013=\"{}\" P20014=\"{}\" P20015=\"{}\"/>",
            esc(&f.function),
            esc(&f.location),
            esc(&f.letters),
            esc(&f.number),
            esc(&f.sub)
        );
        let _ = writeln!(self.out, "      <P144 P144=\"{}\"/>", esc(&f.display));
    }
}

/// Exports `project`.
pub fn write_pxf(project: &Project, opts: &PxfWriteOptions) -> PxfExport {
    let d = opts.dialect;
    let mapper = SymbolMapper::new(Pack::Iec).with_user_table(opts.symbols.clone());
    let mut report = Report::default();
    let mut w = Writer {
        out: String::new(),
        next: 1,
    };

    let _ = writeln!(w.out, "<?xml version=\"1.0\" encoding=\"utf-8\"?>");
    let _ = writeln!(
        w.out,
        "<!-- {WRITER_MARK} for {}. Not yet opened in a real EPLAN. -->",
        d.release
    );
    let pages = project.pages_in_order();
    let _ = writeln!(
        w.out,
        "<EplanPxfRoot Type=\"Project\" Version=\"{}\" PxfVersion=\"{}\" SchemaVersion=\"{}\" NumMainObjects=\"{}\">",
        d.version,
        d.pxf_version,
        d.schema_version,
        pages.len()
    );
    let _ = writeln!(w.out, "  <EplPxfDir/>");
    for (name, id) in d.symbol_libraries {
        let _ = writeln!(w.out, "  <ESymLib Id=\"{id}\" Name=\"{name}\"/>");
    }

    // Symbols grouped by page, in a stable order.
    let mut by_page: BTreeMap<_, Vec<&SymbolInstance>> = BTreeMap::new();
    for s in project.symbols() {
        by_page.entry(s.page).or_default().push(s);
    }

    let mut counts = Counts::default();
    let mut drawn_devices: BTreeSet<DeviceId> = BTreeSet::new();
    let mut drawn_terminals: BTreeSet<TerminalId> = BTreeSet::new();
    let mut odd_sheet = false;
    let mut first_page = true;

    for page in &pages {
        let pid = w.id(4);
        let (func, loc) = (
            levels(&page.structure.function),
            levels(&page.structure.location),
        );
        let _ = writeln!(w.out, "  <O4 A1=\"{pid}\" A1101=\"{}\">", esc(&page.name));
        let _ = writeln!(
            w.out,
            "    <P49 P1100=\"{}\" P1200=\"{}\" P11012=\"{}\" P11013=\"\"/>",
            esc(&func),
            esc(&loc),
            esc(&page.name)
        );
        if let Some(t) = ml(&opts.lang, &page.title) {
            let _ = writeln!(w.out, "    <P11 P11011=\"{}\"/>", esc(&t));
        }
        counts.pages += 1;
        let (sw, sh) = page.format.size_mm();
        if (sw - d.sheet_mm.0).abs() > 0.5 || (sh - d.sheet_mm.1).abs() > 0.5 {
            odd_sheet = true;
        }

        let mut symbols = by_page.remove(&page.id).unwrap_or_default();
        symbols.sort_by_key(|s| s.id);
        for s in symbols {
            match &s.role {
                SymbolRole::Device { device, .. } => {
                    let Some(dev) = project.device(*device) else {
                        continue;
                    };
                    let Some(f) = tag_fields(&dev.tag) else {
                        counts.bad_tags += 1;
                        continue;
                    };
                    let part = dev.parts.first().and_then(|p| project.part(*p));
                    let key = mapper.to_eplan(&s.symbol.0).and_then(|t| t.key);
                    place(&mut w, &mut counts, s, sh, &f, key, |w| {
                        let text = ml(&opts.lang, &dev.description);
                        match (part, text) {
                            (Some(p), text) => {
                                let t =
                                    part_number(&p.manufacturer, &p.type_number, &p.order_number);
                                let _ = writeln!(
                                    w.out,
                                    "      <P11 P20027=\"{}\" P20100_1=\"{}\"/>",
                                    esc(&text.unwrap_or_default()),
                                    esc(&t)
                                );
                            }
                            (None, Some(t)) => {
                                let _ = writeln!(w.out, "      <P11 P20027=\"{}\"/>", esc(&t));
                            }
                            (None, None) => {}
                        }
                    });
                    drawn_devices.insert(*device);
                }
                SymbolRole::Terminal { terminals } => {
                    for tid in terminals {
                        let Some(t) = project.terminal(*tid) else {
                            continue;
                        };
                        let Some(strip) = project.device(t.strip) else {
                            continue;
                        };
                        let Some(f) = tag_fields(&strip.tag) else {
                            counts.bad_tags += 1;
                            continue;
                        };
                        let part = t.part.and_then(|p| project.part(p));
                        let key = mapper.to_eplan(&s.symbol.0).and_then(|t| t.key);
                        place(&mut w, &mut counts, s, sh, &f, key, |w| {
                            write_terminal(w, t, part);
                        });
                        drawn_terminals.insert(*tid);
                    }
                }
                _ => counts.other_symbols += 1,
            }
        }

        // Cables sit on the first page: they have no drawing of their own.
        if first_page {
            first_page = false;
            for c in project.cables() {
                let Some(f) = tag_fields(&c.tag) else {
                    counts.bad_tags += 1;
                    continue;
                };
                let id = w.id(17);
                let _ = writeln!(w.out, "    <O17 A1=\"{id}\">");
                w.tag(&f);
                let text = ml(&opts.lang, &c.description).unwrap_or_else(|| "-".to_string());
                let part = c.part.and_then(|p| project.part(p));
                match part {
                    Some(p) => {
                        let _ = writeln!(
                            w.out,
                            "      <P11 P20011=\"{}\" P20100_1=\"{}\"/>",
                            esc(&text),
                            esc(&part_number(
                                &p.manufacturer,
                                &p.type_number,
                                &p.order_number
                            ))
                        );
                    }
                    None => {
                        let _ = writeln!(w.out, "      <P11 P20011=\"{}\"/>", esc(&text));
                    }
                }
                let _ = writeln!(w.out, "    </O17>");
                counts.cables += 1;
                for core in c.cores.iter().filter(|k| !k.name.trim().is_empty()) {
                    let id = w.id(59);
                    let _ = writeln!(w.out, "    <O59 A1=\"{id}\">");
                    w.tag(&f);
                    let _ = writeln!(w.out, "      <P11 P31004=\"{}\"/>", esc(core.name.trim()));
                    let _ = writeln!(w.out, "    </O59>");
                    counts.cores += 1;
                }
            }
        }
        let _ = writeln!(w.out, "  </O4>");
    }
    let _ = writeln!(w.out, "</EplanPxfRoot>");

    counts.report(
        project,
        &mut report,
        &drawn_devices,
        &drawn_terminals,
        pages.is_empty(),
    );
    report.note(format!(
        "Written for {} (PXF {}). The file has not been opened in a real EPLAN yet; import it into a test project first.",
        d.release, d.pxf_version
    ));
    if odd_sheet {
        report.note(format!(
            "Some pages are not {} x {} mm; positions use each page's own height and were not checked against EPLAN.",
            d.sheet_mm.0, d.sheet_mm.1
        ));
    }
    PxfExport {
        text: w.out,
        report,
    }
}

fn part_number(manufacturer: &str, type_number: &str, order_number: &str) -> String {
    let ty = if type_number.is_empty() {
        order_number
    } else {
        type_number
    };
    if manufacturer.is_empty() {
        ty.to_string()
    } else {
        format!("{manufacturer}.{ty}")
    }
}

fn write_terminal(w: &mut Writer, t: &Terminal, part: Option<&novo_core::Part>) {
    match part {
        Some(p) => {
            let _ = writeln!(
                w.out,
                "      <P11 P20030=\"{}\" P20100_1=\"{}\"/>",
                esc(&t.name),
                esc(&part_number(
                    &p.manufacturer,
                    &p.type_number,
                    &p.order_number
                ))
            );
        }
        None => {
            let _ = writeln!(w.out, "      <P11 P20030=\"{}\"/>", esc(&t.name));
        }
    }
}

/// One `O17` for a placed symbol.
fn place(
    w: &mut Writer,
    counts: &mut Counts,
    s: &SymbolInstance,
    sheet_h: f64,
    f: &TagFields,
    key: Option<SymbolKey>,
    body: impl FnOnce(&mut Writer),
) {
    let id = w.id(17);
    match key {
        Some(k) => {
            counts.with_key += 1;
            let _ = writeln!(
                w.out,
                "    <O17 A1=\"{id}\" A1381=\"{}\" A1382=\"{}\" A1383=\"{}\">",
                k.number, k.variant, k.set
            );
        }
        None => {
            counts.without_key += 1;
            let _ = writeln!(w.out, "    <O17 A1=\"{id}\">");
        }
    }
    w.tag(f);
    body(w);
    let p = s.placement.origin;
    let _ = writeln!(
        w.out,
        "      <S40x1201 A762=\"{}/{}\"/>",
        num(p.x),
        num(sheet_h - p.y)
    );
    let _ = writeln!(w.out, "    </O17>");
}

#[derive(Default)]
struct Counts {
    pages: usize,
    with_key: usize,
    without_key: usize,
    cables: usize,
    cores: usize,
    bad_tags: usize,
    other_symbols: usize,
}

impl Counts {
    fn report(
        &self,
        project: &Project,
        r: &mut Report,
        drawn_devices: &BTreeSet<DeviceId>,
        drawn_terminals: &BTreeSet<TerminalId>,
        no_pages: bool,
    ) {
        r.moved("pages", self.pages);
        r.moved(
            "device and terminal placements with an EPLAN symbol",
            self.with_key,
        );
        r.moved(
            "device and terminal placements without a symbol reference",
            self.without_key,
        );
        r.moved("cables", self.cables);
        r.moved("cable cores", self.cores);
        let undrawn = project
            .devices()
            .filter(|d| d.kind != DeviceKind::TerminalStrip && !drawn_devices.contains(&d.id))
            .count();
        r.lost(
            "devices without a drawn symbol",
            undrawn,
            "EPLAN devices live on drawn symbols; place the device on a page first",
        );
        let undrawn_t = project
            .terminals()
            .filter(|t| !drawn_terminals.contains(&t.id))
            .count();
        r.lost(
            "terminals without a drawn symbol",
            undrawn_t,
            "EPLAN terminals live on drawn symbols; place the terminal on a page first",
        );
        r.lost(
            "tags with more than one device level",
            self.bad_tags,
            "EPLAN tags have one device level",
        );
        r.lost(
            "other symbols",
            self.other_symbols,
            "symbols that are not a device or terminal (interruption points and the like) are not written yet",
        );
        r.lost(
            "wires",
            project.wires().count(),
            "connections are not written yet; EPLAN rebuilds them from the drawing",
        );
        r.lost(
            "texts",
            project.texts().count(),
            "free texts are not written yet",
        );
        if no_pages {
            r.note("The project has no pages, so nothing could be placed.");
        }
        if self.without_key > 0 {
            r.note(format!(
                "{} placements have no EPLAN symbol key in your symbol table and were written without a symbol reference.",
                self.without_key
            ));
        }
    }
}
