//! Builds a [`PxfFile`] from the token stream, one object at a time.

use std::collections::{BTreeMap, HashSet};
use std::io::BufRead;

use super::PxfError;
use super::library::{LibFunction, LibPort, LibSymbol, parse_point, split_pins};
use super::model::*;
use super::stream::{Event, Tokens};
use super::text::MlText;

const MAX_DEPTH: usize = 512;

/// Object types the reader reads into the model.
const KIND_PAGE: u32 = 4;
const KIND_STRUCTURE: u32 = 6;
const KIND_PROJECT: u32 = 14;
const KIND_SYMBOL: u32 = 17;
const KIND_LIB_SYMBOL: u32 = 20;
const KIND_TEXT: u32 = 30;
const KIND_NODE: u32 = 42;
const KIND_LIB_VARIANT: u32 = 45;
const KIND_REPORT_CELL: u32 = 52;
const KIND_REPORT_TEXT: u32 = 53;
const KIND_CORE: u32 = 59;
const KIND_LIB_FUNCTION: u32 = 60;
const KIND_INTERRUPTION: u32 = 70;

/// An open `O<n>` element and the property elements seen inside it.
struct Frame {
    kind: u32,
    attrs: Vec<(String, String)>,
    props: Vec<(String, Vec<(String, String)>)>,
    /// Ordinal of the page this frame is (kind 4) or sits on.
    page: Option<usize>,
    /// Sits inside another object that is not a page (the graphics of a
    /// library symbol), or outside every page.
    library: bool,
    /// The drawn variants (`O45`) of a library symbol, filled as they end.
    variants: Vec<(u32, Vec<LibPort>)>,
}

impl Frame {
    fn own(&self, name: &str) -> Option<&str> {
        find(&self.attrs, name)
    }

    /// The attribute `attr` of the property element `elem`.
    fn prop(&self, elem: &str, attr: &str) -> &str {
        self.props
            .iter()
            .filter(|(n, _)| n == elem)
            .find_map(|(_, a)| find(a, attr))
            .unwrap_or("")
    }

    /// The first attribute called `attr` in any property element: property
    /// ids such as `P20100_1` are unique, so the element does not matter.
    fn any(&self, attr: &str) -> &str {
        self.props
            .iter()
            .find_map(|(_, a)| find(a, attr))
            .or_else(|| self.own(attr))
            .unwrap_or("")
    }

    fn tag(&self) -> PxfTag {
        PxfTag {
            function: self.prop("P150", "P1100").to_string(),
            location: self.prop("P150", "P1200").to_string(),
            prefix: self.prop("P150", "P20012").to_string(),
            letters: self.prop("P150", "P20013").to_string(),
            number: self.prop("P150", "P20014").to_string(),
            sub: self.prop("P150", "P20015").to_string(),
            display: self.any("P144").to_string(),
        }
    }

    /// `x/y` from a position attribute, anywhere in the object.
    fn position(&self) -> Option<(f64, f64)> {
        let v = self
            .props
            .iter()
            .filter(|(n, _)| n.starts_with("S40x"))
            .find_map(|(_, a)| find(a, "A762"))?;
        let (x, y) = v.split_once('/')?;
        Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
    }

    fn matrix(&self) -> [f64; 4] {
        let get = |n: &str, d: f64| {
            self.props
                .iter()
                .find_map(|(e, a)| (e == "S39x761").then(|| find(a, n)).flatten())
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(d)
        };
        [
            get("A751", 1.0),
            get("A752", 0.0),
            get("A753", 0.0),
            get("A754", 1.0),
        ]
    }

    fn int(&self, name: &str) -> i64 {
        self.own(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    fn symbol(&self) -> Option<SymbolRef> {
        let n = self.own("A1381")?.trim().parse().ok()?;
        Some(SymbolRef {
            number: n,
            variant: self
                .own("A1382")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0),
            library: self
                .own("A1383")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0),
        })
    }
}

fn find<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn object_kind(name: &str) -> Option<u32> {
    name.strip_prefix('O')?.parse().ok()
}

/// Reads a PXF project or macro from `r`.
pub(super) fn read<R: BufRead>(r: R) -> Result<PxfFile, PxfError> {
    let mut tokens = Tokens::new(r);
    let mut header: Option<PxfHeader> = None;
    let mut stack: Vec<Frame> = Vec::new();
    // Elements opened outside any object (the root, `EplPxfDir`, ...) and
    // object elements, to match end tags.
    let mut depth = 0usize;
    let mut open_objects: Vec<usize> = Vec::new(); // depth at which each object opened

    let mut pages: Vec<Option<PxfPage>> = Vec::new();
    // Objects written between pages, not inside one, belong to the page
    // before them. Objects inside another object that is not a page (the
    // graphics of a library symbol) never do.
    let mut last_page: Option<usize> = None;
    let mut out = PxfFile {
        header: PxfHeader {
            kind: PxfKind::Other,
            type_name: String::new(),
            version: String::new(),
            pxf_version: String::new(),
            schema_version: String::new(),
            declared_objects: None,
        },
        pages: Vec::new(),
        devices: Vec::new(),
        cores: Vec::new(),
        interruptions: Vec::new(),
        texts: Vec::new(),
        nodes: Vec::new(),
        project: PxfProjectData::default(),
        structures: Vec::new(),
        library: Default::default(),
        skipped: BTreeMap::new(),
        library_objects: BTreeMap::new(),
        duplicate_ids: Vec::new(),
        notes: Vec::new(),
    };
    let mut seen: HashSet<ObjId> = HashSet::new();
    let mut page_objects: BTreeMap<usize, [usize; 4]> = BTreeMap::new();

    while let Some(ev) = tokens.next_event()? {
        match ev {
            Event::Start { name, attrs } => {
                if header.is_none() {
                    if name != "EplanPxfRoot" {
                        return Err(PxfError::NotPxf(name));
                    }
                    let ty = find(&attrs, "Type").unwrap_or("").to_string();
                    header = Some(PxfHeader {
                        kind: match ty.as_str() {
                            "PageMacro" => PxfKind::PageMacro,
                            "" | "Project" | "PxfProject" => PxfKind::Project,
                            _ => PxfKind::Other,
                        },
                        type_name: ty,
                        version: find(&attrs, "Version").unwrap_or("").to_string(),
                        pxf_version: find(&attrs, "PxfVersion").unwrap_or("").to_string(),
                        schema_version: find(&attrs, "SchemaVersion").unwrap_or("").to_string(),
                        declared_objects: find(&attrs, "NumMainObjects")
                            .and_then(|v| v.trim().parse().ok()),
                    });
                    depth = 1;
                    continue;
                }
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(PxfError::TooDeep(MAX_DEPTH));
                }
                if let Some(kind) = object_kind(&name) {
                    let page = if kind == KIND_PAGE {
                        pages.push(None);
                        last_page = Some(pages.len() - 1);
                        last_page
                    } else if stack.is_empty() {
                        last_page
                    } else {
                        stack
                            .iter()
                            .rev()
                            .find_map(|f| f.page.filter(|_| f.kind == KIND_PAGE))
                    };
                    open_objects.push(depth);
                    stack.push(Frame {
                        kind,
                        attrs,
                        props: Vec::new(),
                        page,
                        library: kind != KIND_PAGE && page.is_none(),
                        variants: Vec::new(),
                    });
                } else if let Some(top) = stack.last_mut() {
                    // Only the elements the reader asks about are kept; the
                    // graphics rows of an embedded library are the bulk of a
                    // project and are never looked at.
                    if name.starts_with('P')
                        || name.starts_with("S40x")
                        || name.starts_with("S39x")
                        || name.starts_with("S48x")
                    {
                        top.props.push((name, attrs));
                    }
                }
            }
            Event::End { .. } => {
                if open_objects.last() == Some(&depth) {
                    open_objects.pop();
                    if let Some(frame) = stack.pop() {
                        if frame.kind == KIND_LIB_VARIANT {
                            if let Some(parent) = stack.last_mut() {
                                parent.variants.push(variant_ports(&frame));
                            }
                        } else {
                            finish(frame, &mut out, &mut pages, &mut seen, &mut page_objects);
                        }
                    }
                }
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            Event::Text(_) => {}
        }
    }
    let Some(header) = header else {
        return Err(PxfError::Empty);
    };
    if depth > 0 {
        return Err(PxfError::Truncated(tokens.line()));
    }
    out.header = header;
    // A page start with no end tag cannot happen here (depth checked), so
    // every slot is filled; keep the order of appearance.
    out.pages = pages.into_iter().flatten().collect();
    for (i, [all, symbols, nodes, cells]) in page_objects {
        if let Some(p) = out.pages.get_mut(i) {
            p.objects = all;
            p.symbols = symbols;
            p.nodes = nodes;
            p.report_cells = cells;
        }
    }
    // A cable drawn without a cable text is still a cable when a core says
    // it belongs to it.
    let cable_ids: std::collections::BTreeSet<ObjId> =
        out.cores.iter().filter_map(|c| c.cable_ref).collect();
    for d in &mut out.devices {
        if d.role == DeviceRole::Device && d.id.is_some_and(|i| cable_ids.contains(&i)) {
            d.role = DeviceRole::Cable;
        }
    }
    finish_notes(&mut out);
    Ok(out)
}

fn finish(
    f: Frame,
    out: &mut PxfFile,
    pages: &mut [Option<PxfPage>],
    seen: &mut HashSet<ObjId>,
    page_objects: &mut BTreeMap<usize, [usize; 4]>,
) {
    let id = f.own("A1").and_then(ObjId::parse);
    if let Some(i) = id
        && !seen.insert(i)
        && !out.duplicate_ids.contains(&i)
    {
        out.duplicate_ids.push(i);
    }
    if f.kind != KIND_PAGE
        && let Some(p) = f.page
    {
        let c = page_objects.entry(p).or_default();
        c[0] += 1;
        match f.kind {
            KIND_SYMBOL => c[1] += 1,
            KIND_NODE | KIND_INTERRUPTION => c[2] += 1,
            KIND_REPORT_CELL | KIND_REPORT_TEXT => c[3] += 1,
            _ => {}
        }
    }
    if f.library
        && !matches!(
            f.kind,
            KIND_STRUCTURE | KIND_PROJECT | KIND_LIB_SYMBOL | KIND_LIB_FUNCTION
        )
    {
        *out.library_objects.entry(f.kind).or_default() += 1;
        return;
    }
    match f.kind {
        KIND_PROJECT => {
            out.project = PxfProjectData {
                number: f.any("P10013").to_string(),
                title: MlText::parse(f.any("P10011")),
            };
        }
        KIND_STRUCTURE => {
            let code = f.own("A82").unwrap_or("").trim();
            if !code.is_empty() {
                out.structures.push(PxfStructure {
                    aspect: f
                        .own("A81")
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0),
                    code: code.to_string(),
                    title: MlText::parse(f.any("P1002")),
                });
            }
        }
        KIND_LIB_SYMBOL => {
            let mut variants = BTreeMap::new();
            for (v, ports) in &f.variants {
                variants.entry(*v).or_insert_with(|| ports.clone());
            }
            out.library.add_symbol(LibSymbol {
                function_def: f.int("A242"),
                library_id: f.int("A247"),
                key: (f.int("A1381"), f.int("A1382"), f.int("A1383")),
                code: f.own("A241").unwrap_or("").to_string(),
                name: MlText::parse(f.any("P16011")),
                variants,
            });
        }
        KIND_LIB_FUNCTION => {
            let rows = |name: &str| -> Vec<String> {
                f.own(name)
                    .unwrap_or("")
                    .split('\t')
                    .flat_map(|row| row.split('\n'))
                    .map(|p| p.trim().to_string())
                    .collect()
            };
            let letters = f
                .own("A1044")
                .and_then(|v| v.split('\n').next())
                .unwrap_or("")
                .trim()
                .to_string();
            out.library.add_function(LibFunction {
                key: (f.int("A1381"), f.int("A1382"), f.int("A1383")),
                name: MlText::parse(f.own("A1041").unwrap_or("")),
                letters,
                pins: rows("A1045"),
            });
        }
        KIND_NODE => out.nodes.push(PxfNode {
            id,
            page: f.page,
            kind: f.int("A1262"),
            variant: f.int("A1263"),
            position: f.position(),
        }),
        KIND_PAGE => {
            let number = match f.prop("P49", "P11012") {
                "" => f.own("A1101").unwrap_or(""),
                n => n,
            };
            let title = f
                .props
                .iter()
                .filter(|(n, _)| n == "P11")
                .find_map(|(_, a)| find(a, "P11011"))
                .map(MlText::parse)
                .unwrap_or_default();
            if let Some(slot) = f.page.and_then(|p| pages.get_mut(p)) {
                *slot = Some(PxfPage {
                    id,
                    function: f.prop("P49", "P1100").to_string(),
                    location: f.prop("P49", "P1200").to_string(),
                    number: number.to_string(),
                    suffix: f.prop("P49", "P11013").to_string(),
                    title,
                    objects: 0,
                    symbols: 0,
                    nodes: 0,
                    report_cells: 0,
                });
            }
        }
        KIND_CORE => out.cores.push(PxfCore {
            id,
            page: f.page,
            cable: f.tag(),
            cable_ref: f.own("R1031").and_then(ObjId::parse),
            number: f.any("P31004").to_string(),
            position: f.position(),
        }),
        KIND_INTERRUPTION => out.interruptions.push(PxfInterruption {
            id,
            page: f.page,
            tag: f.tag(),
            text: f.any("P24810").to_string(),
            position: f.position(),
            variant: f.int("A1263"),
        }),
        KIND_TEXT => {
            // The text is the object's own `A511` and its position `A501`;
            // the height and angle (radians, counter-clockwise) are on the
            // `S54x505` element. Older material kept the text in a property:
            // then take the first value with the multi-language shape.
            let text = f
                .own("A511")
                .filter(|v| MlText::looks_like(v))
                .or_else(|| {
                    f.props
                        .iter()
                        .flat_map(|(_, a)| a.iter())
                        .map(|(_, v)| v.as_str())
                        .find(|v| MlText::looks_like(v))
                })
                .map(MlText::parse)
                .unwrap_or_default();
            let position = f.position().or_else(|| {
                let (x, y) = f.own("A501")?.split_once('/')?;
                Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
            });
            let style = |n: &str| -> Option<f64> {
                f.props
                    .iter()
                    .find_map(|(e, a)| (e == "S54x505").then(|| find(a, n)).flatten())
                    .and_then(|v| v.trim().parse().ok())
            };
            out.texts.push(PxfText {
                id,
                page: f.page,
                text,
                position,
                height: style("A961").filter(|h| *h > 0.0),
                angle: style("A962").unwrap_or(0.0),
            });
        }
        kind => {
            let tag = f.tag();
            if tag.is_empty() {
                *out.skipped.entry(kind).or_default() += 1;
                return;
            }
            let terminal_text = f.any("P20030").to_string();
            let cable_text = f.any("P20011").to_string();
            let role = if !terminal_text.is_empty() {
                DeviceRole::Terminal
            } else if !cable_text.is_empty() {
                DeviceRole::Cable
            } else {
                DeviceRole::Device
            };
            out.devices.push(PxfDevice {
                id,
                page: f.page,
                role,
                tag,
                type_text: MlText::parse(f.any("P20027")),
                part_number: f.any("P20100_1").to_string(),
                terminal_text,
                cable_text,
                symbol: f.symbol(),
                position: f.position(),
                function_def: f.int("A1262"),
                library_id: f.int("A1261"),
                variant: f.int("A1263"),
                pins: split_pins(f.own("A185").unwrap_or("")),
                matrix: f.matrix(),
            });
        }
    }
}

/// The connection points of an `O45` variant: `S48x823` rows with the index
/// (`A861`), the offset (`A862`, `x/y`) and the directions (`A863`).
fn variant_ports(f: &Frame) -> (u32, Vec<LibPort>) {
    let ports = f
        .props
        .iter()
        .filter(|(n, _)| n == "S48x823")
        .filter_map(|(_, a)| {
            let (x, y) = parse_point(find(a, "A862")?)?;
            Some(LibPort {
                index: find(a, "A861")?.trim().parse().ok()?,
                x,
                y,
                dirs: find(a, "A863")?.trim().parse().ok()?,
            })
        })
        .collect();
    (f.int("A822").max(0) as u32, ports)
}

fn finish_notes(out: &mut PxfFile) {
    let h = &out.header;
    if h.pxf_version != super::TESTED_PXF_VERSION {
        out.notes.push(format!(
            "The file is PXF version {}; this reader was written for {}. Check the counts.",
            if h.pxf_version.is_empty() {
                "unknown"
            } else {
                &h.pxf_version
            },
            super::TESTED_PXF_VERSION
        ));
    }
    if let Some(n) = h.declared_objects {
        let found = out.pages.len() as u64;
        if n != found && h.kind == PxfKind::Project {
            out.notes.push(format!(
                "The file declares {n} main objects; {found} pages were found."
            ));
        }
    }
}
