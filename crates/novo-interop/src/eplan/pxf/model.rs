//! What a PXF file holds, as plain values. Nothing here refers to the XML.

use std::collections::BTreeMap;

use super::text::MlText;

/// `4/123`: object type and number. Numbers are unique in the project;
/// devices and cable cores share one number space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjId {
    pub kind: u32,
    pub number: u64,
}

impl ObjId {
    pub fn parse(s: &str) -> Option<Self> {
        let (k, n) = s.trim().split_once('/')?;
        Some(Self {
            kind: k.parse().ok()?,
            number: n.parse().ok()?,
        })
    }
}

/// What kind of file the root says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PxfKind {
    /// A project export (`.epj`).
    Project,
    /// A page macro (`.emp`).
    PageMacro,
    /// Some other `Type` value.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PxfHeader {
    pub kind: PxfKind,
    /// The raw `Type` attribute.
    pub type_name: String,
    pub version: String,
    pub pxf_version: String,
    pub schema_version: String,
    /// `NumMainObjects` as the file states it.
    pub declared_objects: Option<u64>,
}

/// A device tag in its parts: function `=`, location `+`, then the device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PxfTag {
    pub function: String,
    pub location: String,
    pub prefix: String,
    pub letters: String,
    pub number: String,
    pub sub: String,
    /// The tag as EPLAN shows it (`P144`), when the file has it.
    pub display: String,
}

impl PxfTag {
    pub fn is_empty(&self) -> bool {
        self.letters.is_empty() && self.number.is_empty() && self.display.is_empty()
    }

    /// `-K1.2`: the device part. A prefix that is not a sign (`+`, `-`, `=`)
    /// is read as part of the name and the sign is `-`. A sub index is
    /// written after a dot.
    pub fn device_name(&self) -> String {
        let signs = ['-', '+', '=', '&', '#'];
        let mut s = String::new();
        if self
            .prefix
            .chars()
            .next()
            .is_some_and(|c| signs.contains(&c))
        {
            s.push_str(&self.prefix);
        } else {
            s.push('-');
            s.push_str(&self.prefix);
        }
        s.push_str(&self.letters);
        s.push_str(&self.number);
        if !self.sub.is_empty() {
            if !self.sub.starts_with('.') {
                s.push('.');
            }
            s.push_str(&self.sub);
        }
        s
    }

    /// `=A1+CAB1-K1`, KoperCAD's order (IEC 81346). Empty parts are left out.
    pub fn full(&self) -> String {
        let mut s = String::new();
        let f = self.function.trim_start_matches('=');
        if !f.is_empty() {
            s.push('=');
            s.push_str(f);
        }
        let l = self.location.trim_start_matches('+');
        if !l.is_empty() {
            s.push('+');
            s.push_str(l);
        }
        s.push_str(&self.device_name());
        s
    }
}

/// `SCHNEI.GV2P10`: EPLAN's maker and type, split at the first dot.
pub fn split_part_number(p: &str) -> (&str, &str) {
    p.split_once('.').unwrap_or(("", p))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PxfPage {
    pub id: Option<ObjId>,
    pub function: String,
    pub location: String,
    pub number: String,
    pub suffix: String,
    pub title: MlText,
    /// Objects found on the page (all kinds).
    pub objects: usize,
    /// Symbol placements (`O17`) on the page.
    pub symbols: usize,
    /// Connection nodes (`O42`) and interruption points (`O70`).
    pub nodes: usize,
    /// Report cells (`O52`, `O53`): a generated report page is made of them.
    pub report_cells: usize,
}

/// What a page is, decided from what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageClass {
    /// Carries symbols, nodes or interruption points: a circuit diagram.
    Schematic,
    /// Made of report cells: a list EPLAN generated (contents, parts list,
    /// terminal plan). KoperCAD generates its own from the data.
    Report,
    /// Anything else: a cover, a layout drawing, an empty page.
    Other,
}

impl PxfPage {
    pub fn class(&self) -> PageClass {
        if self.symbols > 0 || self.nodes > 0 {
            PageClass::Schematic
        } else if self.report_cells > 0 {
            PageClass::Report
        } else {
            PageClass::Other
        }
    }

    /// `=A1+CAB1/12`-style label.
    pub fn label(&self) -> String {
        let mut s = String::new();
        if !self.function.is_empty() {
            s.push('=');
            s.push_str(self.function.trim_start_matches('='));
        }
        if !self.location.is_empty() {
            s.push('+');
            s.push_str(self.location.trim_start_matches('+'));
        }
        s.push('/');
        s.push_str(&self.number);
        s.push_str(&self.suffix);
        s
    }
}

/// What a tagged object is, decided from its texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRole {
    Device,
    /// Carries a terminal text.
    Terminal,
    /// Carries a cable text and no terminal text.
    Cable,
}

/// The symbol a device uses, as EPLAN numbers it (`A1381` to `A1383`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolRef {
    pub number: i64,
    pub variant: i64,
    /// Library set id; resolve through the file's `ESymLib` rows.
    pub library: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PxfDevice {
    pub id: Option<ObjId>,
    /// Index into [`PxfFile::pages`].
    pub page: Option<usize>,
    pub role: DeviceRole,
    pub tag: PxfTag,
    pub type_text: MlText,
    /// EPLAN part number, `MAKER.TYPE`.
    pub part_number: String,
    pub terminal_text: String,
    pub cable_text: String,
    pub symbol: Option<SymbolRef>,
    /// Millimetres, y up, origin bottom left of the sheet.
    pub position: Option<(f64, f64)>,
    /// How the symbol is looked up in the file's library: the function
    /// definition (`A1262`), the library id (`A1261`) and the drawn variant
    /// (`A1263`, which of the symbol's orientations).
    pub function_def: i64,
    pub library_id: i64,
    pub variant: i64,
    /// Pin designations of this placement (`A185`, `;` separated), in the
    /// order of the symbol's connection points.
    pub pins: Vec<String>,
    /// The placement's 2x2 matrix (`S39x761`, `A751`..`A754`); the identity
    /// when the file has none. The variant already carries the symbol's
    /// orientation, so this is expected to be the identity.
    pub matrix: [f64; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct PxfCore {
    pub id: Option<ObjId>,
    pub page: Option<usize>,
    /// The cable it belongs to, when the core names it.
    pub cable: PxfTag,
    /// The cable's own object (`R1031`): exact, unlike the tag, which a
    /// core may carry without the function level.
    pub cable_ref: Option<ObjId>,
    pub number: String,
    pub position: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PxfInterruption {
    pub id: Option<ObjId>,
    pub page: Option<usize>,
    pub tag: PxfTag,
    pub text: String,
    pub position: Option<(f64, f64)>,
    /// The drawn variant (`A1263`): 1 and 3 modulo 4 point vertically.
    pub variant: i64,
}

/// A corner or T joint (`O42`).
#[derive(Debug, Clone, PartialEq)]
pub struct PxfNode {
    pub id: Option<ObjId>,
    pub page: Option<usize>,
    pub kind: i64,
    pub variant: i64,
    pub position: Option<(f64, f64)>,
}

/// The project's own data (`O14`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PxfProjectData {
    /// `P10013`: the project number the exporting project carries.
    pub number: String,
    /// `P10011`: the project title.
    pub title: MlText,
}

/// A row of the structure list (`O6`): a function or location code and its
/// title.
#[derive(Debug, Clone, PartialEq)]
pub struct PxfStructure {
    /// `1100` for a function (`=`), `1200` for a location (`+`).
    pub aspect: u32,
    pub code: String,
    pub title: MlText,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PxfText {
    pub id: Option<ObjId>,
    pub page: Option<usize>,
    pub text: MlText,
    pub position: Option<(f64, f64)>,
    /// Character height in millimetres, when the file gives one.
    pub height: Option<f64>,
    /// Angle in radians, counter-clockwise, y up.
    pub angle: f64,
}

/// Everything read from one PXF file.
#[derive(Debug, Clone, PartialEq)]
pub struct PxfFile {
    pub header: PxfHeader,
    pub pages: Vec<PxfPage>,
    pub devices: Vec<PxfDevice>,
    pub cores: Vec<PxfCore>,
    pub interruptions: Vec<PxfInterruption>,
    pub texts: Vec<PxfText>,
    /// Lines, rectangles, arcs, polygons and location boxes on pages.
    pub shapes: Vec<super::graphics::PxfShape>,
    pub nodes: Vec<PxfNode>,
    pub project: PxfProjectData,
    pub structures: Vec<PxfStructure>,
    /// The symbol and function definitions the file embeds.
    pub library: super::library::PxfLibrary,
    /// Objects the reader does not carry, by object type.
    pub skipped: BTreeMap<u32, usize>,
    /// Objects that sit in the embedded library, not on a page, by object
    /// type (graphics of EPLAN's own symbols). Never carried.
    pub library_objects: BTreeMap<u32, usize>,
    /// Ids that occur twice (EPLAN would refuse such a file on import).
    pub duplicate_ids: Vec<ObjId>,
    /// Things worth telling the person.
    pub notes: Vec<String>,
}

impl PxfFile {
    pub fn devices_of(&self, role: DeviceRole) -> impl Iterator<Item = &PxfDevice> {
        self.devices.iter().filter(move |d| d.role == role)
    }
}

/// Interruption points with the same full name pair up two by two in file
/// (page) order; an odd one stays alone. Returns the pairs and the lonely
/// ones, as indexes into `points`.
pub fn pair_interruptions(points: &[PxfInterruption]) -> (Vec<(usize, usize)>, Vec<usize>) {
    let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, p) in points.iter().enumerate() {
        by_name.entry(p.tag.full()).or_default().push(i);
    }
    let mut pairs = Vec::new();
    let mut alone = Vec::new();
    for idx in by_name.values() {
        for c in idx.chunks(2) {
            match c {
                [a, b] => pairs.push((*a, *b)),
                [a] => alone.push(*a),
                _ => {}
            }
        }
    }
    pairs.sort_unstable();
    alone.sort_unstable();
    (pairs, alone)
}
