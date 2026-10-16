//! Builds a [`PxfFile`] from the token stream, one object at a time.

use std::collections::{BTreeMap, HashSet};
use std::io::BufRead;

use super::PxfError;
use super::model::*;
use super::stream::{Event, Tokens};
use super::text::MlText;

const MAX_DEPTH: usize = 512;

/// Object types the reader reads into the model.
const KIND_PAGE: u32 = 4;
const KIND_TEXT: u32 = 30;
const KIND_CORE: u32 = 59;
const KIND_INTERRUPTION: u32 = 70;

/// An open `O<n>` element and the property elements seen inside it.
struct Frame {
    kind: u32,
    attrs: Vec<(String, String)>,
    props: Vec<(String, Vec<(String, String)>)>,
    /// Ordinal of the page this frame is (kind 4) or sits on.
    page: Option<usize>,
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
        skipped: BTreeMap::new(),
        duplicate_ids: Vec::new(),
        notes: Vec::new(),
    };
    let mut seen: HashSet<ObjId> = HashSet::new();
    let mut page_objects: BTreeMap<usize, usize> = BTreeMap::new();

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
                    } else {
                        stack
                            .iter()
                            .rev()
                            .find_map(|f| f.page.filter(|_| f.kind == KIND_PAGE))
                            .or(last_page)
                    };
                    open_objects.push(depth);
                    stack.push(Frame {
                        kind,
                        attrs,
                        props: Vec::new(),
                        page,
                    });
                } else if let Some(top) = stack.last_mut() {
                    top.props.push((name, attrs));
                }
            }
            Event::End { .. } => {
                if open_objects.last() == Some(&depth) {
                    open_objects.pop();
                    if let Some(frame) = stack.pop() {
                        finish(frame, &mut out, &mut pages, &mut seen, &mut page_objects);
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
    for (i, n) in page_objects {
        if let Some(p) = out.pages.get_mut(i) {
            p.objects = n;
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
    page_objects: &mut BTreeMap<usize, usize>,
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
        *page_objects.entry(p).or_default() += 1;
    }
    match f.kind {
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
                });
            }
        }
        KIND_CORE => out.cores.push(PxfCore {
            id,
            page: f.page,
            cable: f.tag(),
            number: f.any("P31004").to_string(),
        }),
        KIND_INTERRUPTION => out.interruptions.push(PxfInterruption {
            id,
            page: f.page,
            tag: f.tag(),
            text: f.any("P24810").to_string(),
        }),
        KIND_TEXT => {
            // The property that holds the text is not pinned down by the
            // material this reader was written from: take the first value
            // with the multi-language shape.
            let text = f
                .props
                .iter()
                .flat_map(|(_, a)| a.iter())
                .map(|(_, v)| v.as_str())
                .find(|v| MlText::looks_like(v))
                .map(MlText::parse)
                .unwrap_or_default();
            out.texts.push(PxfText {
                id,
                page: f.page,
                text,
                position: f.position(),
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
            });
        }
    }
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
